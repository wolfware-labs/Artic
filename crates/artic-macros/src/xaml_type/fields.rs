use crate::xaml_type::attributes::{FieldAttributes, TypeAttributes, parse_field_attributes, parse_type_attributes};
use syn::{Data, DeriveInput, Error, Field, Fields, GenericArgument, Ident, PathArguments, Type};

/// A struct read for the derive: its attributes and its fields, sorted by role.
pub struct Analysis<'a> {
  pub ident: &'a Ident,
  pub attributes: TypeAttributes,
  pub xaml_fields: Vec<XamlField<'a>>,
  pub other_fields: Vec<&'a Ident>,
}

/// A field marked `#[xaml]`.
pub struct XamlField<'a> {
  pub ident: &'a Ident,
  pub ty: &'a Type,
  pub role: Role<'a>,
  pub attributes: FieldAttributes,
}

/// What a `#[xaml]` field is, decided from how its type is written.
pub enum Role<'a> {
  /// `Property<T>`. `inner` is `T`; `value` is `T`, or the `X` of `Option<X>`, which is what text
  /// or an element sets.
  Property { inner: &'a Type, value: &'a Type },
  /// `Event<A>`, with `A` the arguments handlers receive.
  Event { args: &'a Type },
  /// Any other type, which must be a collection of child elements.
  Collection,
  /// `#[xaml(flatten)]`: a property group whose properties are listed as the type's.
  Group,
}

/// Reads the struct's attributes and sorts its fields by role.
pub fn analyze(input: &DeriveInput) -> syn::Result<Analysis<'_>> {
  let Data::Struct(data) = &input.data else {
    return Err(Error::new_spanned(
      &input.ident,
      "`XamlType` can only be derived for a struct",
    ));
  };
  let fields = match &data.fields {
    Fields::Named(fields) => fields.named.iter().collect(),
    Fields::Unit => Vec::new(),
    Fields::Unnamed(_) => {
      return Err(Error::new_spanned(
        &data.fields,
        "`XamlType` needs a struct with named fields",
      ));
    }
  };
  if !input.generics.params.is_empty() {
    return Err(Error::new_spanned(
      &input.generics,
      "`XamlType` can't be derived for a generic struct",
    ));
  }
  let attributes = parse_type_attributes(input)?;
  let mut xaml_fields = Vec::new();
  let mut other_fields = Vec::new();
  for field in fields {
    let ident = field.ident.as_ref().expect("named fields have names");
    match parse_field_attributes(field)? {
      Some(field_attributes) => xaml_fields.push(classify(field, ident, field_attributes)?),
      None => {
        reject_unmarked_member(field)?;
        other_fields.push(ident);
      }
    }
  }
  check_attached(&attributes, &xaml_fields)?;
  check_positional(&input.ident, &attributes, &xaml_fields)?;
  check_content(&xaml_fields)?;
  Ok(Analysis {
    ident: &input.ident,
    attributes,
    xaml_fields,
    other_fields,
  })
}

/// Rejects a second field marked `#[xaml(content)]`, and the mark on an event.
pub fn check_content(xaml_fields: &[XamlField]) -> syn::Result<()> {
  let mut marked = xaml_fields
    .iter()
    .filter_map(|field| field.attributes.content.map(|span| (field, span)));
  if let Some((field, span)) = marked.next()
    && matches!(field.role, Role::Event { .. })
  {
    return Err(Error::new(
      span,
      "`content` goes on a `Property` or a collection field, not an event",
    ));
  }
  if let Some((_, span)) = marked.next() {
    return Err(Error::new(span, "a type has at most one `#[xaml(content)]` member"));
  }
  Ok(())
}

/// Rejects two attached properties with one name, and an attached property whose helpers would
/// clash with a property's methods, which are named after its field.
fn check_attached(attributes: &TypeAttributes, xaml_fields: &[XamlField]) -> syn::Result<()> {
  for (index, attached) in attributes.attached.iter().enumerate() {
    let name = attached.name.value();
    if attributes.attached[..index]
      .iter()
      .any(|earlier| earlier.name.value() == name)
    {
      return Err(Error::new_spanned(
        &attached.name,
        format_args!("this type already declares the attached property `{name}`"),
      ));
    }
    let getter = snake_case(&name);
    let setter = format!("set_{getter}");
    let clash = xaml_fields
      .iter()
      .filter(|field| matches!(field.role, Role::Property { .. }))
      .find(|field| field.ident == getter.as_str() || field.ident == setter.as_str());
    if let Some(field) = clash {
      return Err(Error::new_spanned(
        &attached.name,
        format_args!(
          "the attached property `{name}`'s helpers `{getter}` and `{setter}` clash with the property `{}`; rename one with `name = ...`",
          field.ident
        ),
      ));
    }
  }
  Ok(())
}

/// Rejects a positional argument that isn't a property, or that is listed twice.
fn check_positional(owner: &Ident, attributes: &TypeAttributes, xaml_fields: &[XamlField]) -> syn::Result<()> {
  let Some(positional) = &attributes.markup_extension else {
    return Ok(());
  };
  for (index, argument) in positional.iter().enumerate() {
    let is_property = xaml_fields
      .iter()
      .any(|field| field.ident == argument && matches!(field.role, Role::Property { .. }));
    if !is_property {
      return Err(Error::new_spanned(
        argument,
        format_args!("`positional({argument})` names no property of `{owner}`"),
      ));
    }
    if positional[..index].contains(argument) {
      return Err(Error::new_spanned(
        argument,
        format_args!("`{argument}` is listed twice in `positional(...)`"),
      ));
    }
  }
  Ok(())
}

/// The field's name in markup: its `name` override, or the field name in PascalCase.
pub fn xaml_name(field: &XamlField) -> String {
  match &field.attributes.name {
    Some(name) => name.value(),
    None => pascal_case(&field.ident.to_string()),
  }
}

/// Decides a `#[xaml]` field's role from its attributes and how its type is written.
pub fn classify<'a>(field: &'a Field, ident: &'a Ident, attributes: FieldAttributes) -> syn::Result<XamlField<'a>> {
  let role = if attributes.flatten.is_some() {
    Role::Group
  } else if let Some(value) = generic_argument(&field.ty, "Property") {
    Role::Property {
      inner: value,
      value: generic_argument(value, "Option").unwrap_or(value),
    }
  } else if let Some(args) = generic_argument(&field.ty, "Event") {
    Role::Event { args }
  } else {
    Role::Collection
  };
  if matches!(role, Role::Event { .. } | Role::Collection) {
    let property_only =
      attributes.default.is_some() || attributes.key.is_some() || attributes.affects.is_some() || attributes.inherits.is_some();
    if property_only {
      return Err(Error::new_spanned(
        &field.ty,
        "`default`, `key`, `affects` and `inherits` only apply to `Property` fields",
      ));
    }
  }
  Ok(XamlField {
    ident,
    ty: &field.ty,
    role,
    attributes,
  })
}

/// A `Property` or `Event` field must be marked `#[xaml]`: the derive builds properties with their
/// keys, and an unmarked one would have none.
pub fn reject_unmarked_member(field: &Field) -> syn::Result<()> {
  for wrapper in ["Property", "Event"] {
    if generic_argument(&field.ty, wrapper).is_some() {
      return Err(Error::new_spanned(
        &field.ty,
        format_args!("add `#[xaml]` to this `{wrapper}` field so the derive can create it"),
      ));
    }
  }
  Ok(())
}

/// If `ty` is written `Wrapper<T>`, possibly with a path in front, returns `T`.
pub fn generic_argument<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
  let Type::Path(path) = ty else {
    return None;
  };
  let segment = path.path.segments.last()?;
  if segment.ident != wrapper {
    return None;
  }
  let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
    return None;
  };
  match arguments.args.first()? {
    GenericArgument::Type(argument) if arguments.args.len() == 1 => Some(argument),
    _ => None,
  }
}

/// `ColumnSpan` as `column_span`: the name of a generated helper.
pub fn snake_case(name: &str) -> String {
  let mut snake = String::with_capacity(name.len() + 4);
  for (index, character) in name.chars().enumerate() {
    if character.is_uppercase() && index > 0 {
      snake.push('_');
    }
    snake.extend(character.to_lowercase());
  }
  snake
}

fn pascal_case(name: &str) -> String {
  name
    .trim_start_matches("r#")
    .split('_')
    .filter(|part| !part.is_empty())
    .map(|part| {
      let mut chars = part.chars();
      chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect::<String>())
        .unwrap_or_default()
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;
  use quote::ToTokens;
  use syn::parse_quote;

  /// Returns the source text of `ty`, for comparing types.
  fn text(ty: &Type) -> String {
    ty.to_token_stream().to_string()
  }

  // Each `#[xaml]` field's role follows how its type is written, looking through `Option` for a
  // property's value; unmarked fields are kept apart.
  #[test]
  fn fields_are_sorted_by_role() {
    let input: DeriveInput = parse_quote! {
      #[xaml(namespace = "https://artic.rs/ui")]
      struct Border {
        layout: Layout,
        #[xaml(content)] child: Property<Option<NodeId>>,
        #[xaml(default = Thickness::ZERO)] padding: artic::Property<Thickness>,
        #[xaml] pointer_pressed: Event<PointerEvent>,
        #[xaml] items: Vec<NodeId>,
      }
    };

    let analysis = analyze(&input).expect("valid struct");

    let roles: Vec<String> = analysis
      .xaml_fields
      .iter()
      .map(|field| match &field.role {
        Role::Property { value, .. } => format!("property {}", text(value)),
        Role::Event { args } => format!("event {}", text(args)),
        Role::Collection => "collection".to_owned(),
        Role::Group => "group".to_owned(),
      })
      .collect();
    assert_eq!(
      roles,
      ["property NodeId", "property Thickness", "event PointerEvent", "collection"]
    );
    assert_eq!(analysis.other_fields, ["layout"]);
  }

  // `#[xaml(flatten)]` makes a field a group, whatever its type looks like.
  #[test]
  fn flatten_makes_a_group() {
    let input: DeriveInput = parse_quote! {
      #[xaml(namespace = "https://artic.rs/ui")]
      struct Border { #[xaml(flatten)] layout: Layout }
    };

    let analysis = analyze(&input).expect("valid struct");

    assert!(matches!(analysis.xaml_fields[0].role, Role::Group));
  }

  // A field's markup name is its name in PascalCase unless `name` overrides it.
  #[test]
  fn xaml_names() {
    let input: DeriveInput = parse_quote! {
      #[xaml(namespace = "https://artic.rs/ui")]
      struct Border {
        #[xaml] corner_radius: Property<f32>,
        #[xaml(name = "BorderBrush")] brush: Property<f32>,
      }
    };

    let analysis = analyze(&input).expect("valid struct");

    let names: Vec<String> = analysis.xaml_fields.iter().map(xaml_name).collect();
    assert_eq!(names, ["CornerRadius", "BorderBrush"]);
  }

  // Attached property names become snake_case helper names.
  #[test]
  fn snake_case_names() {
    let names: Vec<String> = ["Row", "ColumnSpan", "Name"].into_iter().map(snake_case).collect();
    assert_eq!(names, ["row", "column_span", "name"]);
  }

  // An event can't be content: content is a value or child elements.
  #[test]
  fn content_on_an_event_is_an_error() {
    let input: DeriveInput = parse_quote! {
      #[xaml(namespace = "https://artic.rs/ui")]
      struct Button { #[xaml(content)] click: Event<ClickEvent> }
    };

    let error = analyze(&input).err().expect("content on an event");

    assert!(error.to_string().contains("not an event"));
  }

  // A `Property` field without `#[xaml]` is rejected, since the derive must create it with a key.
  #[test]
  fn unmarked_property_is_an_error() {
    let input: DeriveInput = parse_quote! {
      #[xaml(namespace = "https://artic.rs/ui")]
      struct Border { padding: Property<f32> }
    };

    let error = analyze(&input).err().expect("unmarked property");

    assert!(error.to_string().starts_with("add `#[xaml]` to this `Property` field"));
  }

  // Property settings on an event field are rejected.
  #[test]
  fn property_settings_on_an_event_are_an_error() {
    let input: DeriveInput = parse_quote! {
      #[xaml(namespace = "https://artic.rs/ui")]
      struct Button { #[xaml(affects = render)] clicked: Event<ClickArgs> }
    };

    let error = analyze(&input).err().expect("affects on an event");

    assert!(error.to_string().contains("only apply to `Property` fields"));
  }
}
