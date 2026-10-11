use proc_macro2::Span;
use syn::meta::ParseNestedMeta;
use syn::{DeriveInput, Error, Expr, Field, Ident, LitStr, Meta, Path, Token, Type};

/// What `#[xaml(...)]` on the struct says.
#[cfg_attr(test, derive(Debug))]
pub struct TypeAttributes {
  pub namespace: LitStr,
  pub content: Option<Ident>,
  pub whitespace: Whitespace,
  pub crate_path: Option<Path>,
  pub attached: Vec<Attached>,
  /// The properties set by position, in order, if the type is a markup extension.
  pub markup_extension: Option<Vec<Ident>>,
}

/// One `attached(...)` entry on the struct: an attached property the type owns.
#[cfg_attr(test, derive(Debug))]
pub struct Attached {
  pub name: LitStr,
  pub ty: Type,
  pub default: Option<Expr>,
  pub affects: Affects,
  pub target: Target,
}

/// Where an attached property may be set, from `target = children | any`.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Target {
  Any,
  Children,
}

/// How the parser treats whitespace in the element's text content.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Whitespace {
  Collapse,
  Significant,
}

/// What `#[xaml(...)]` on a field says.
#[derive(Default)]
#[cfg_attr(test, derive(Debug))]
pub struct FieldAttributes {
  pub default: Option<Expr>,
  pub key: Option<Path>,
  pub affects: Option<Affects>,
  pub inherits: Option<Span>,
  pub name: Option<LitStr>,
  pub flatten: Option<Span>,
}

/// The stages listed in `affects = layout | render`.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct Affects {
  pub layout: bool,
  pub render: bool,
}

/// Reads the struct's `#[xaml(...)]` attributes.
pub fn parse_type_attributes(input: &DeriveInput) -> syn::Result<TypeAttributes> {
  let mut namespace = None;
  let mut content = None;
  let mut whitespace = Whitespace::Collapse;
  let mut crate_path = None;
  let mut attached = Vec::new();
  let mut markup_extension = None;
  for attribute in input.attrs.iter().filter(|attribute| attribute.path().is_ident("xaml")) {
    attribute.parse_nested_meta(|meta| {
      if meta.path.is_ident("namespace") {
        namespace = Some(meta.value()?.parse::<LitStr>()?);
      } else if meta.path.is_ident("content") {
        content = Some(meta.value()?.parse::<Ident>()?);
      } else if meta.path.is_ident("whitespace") {
        let value: LitStr = meta.value()?.parse()?;
        whitespace = match value.value().as_str() {
          "collapse" => Whitespace::Collapse,
          "significant" => Whitespace::Significant,
          _ => return Err(Error::new_spanned(value, "expected \"collapse\" or \"significant\"")),
        };
      } else if meta.path.is_ident("crate") {
        let value: LitStr = meta.value()?.parse()?;
        crate_path = Some(value.parse::<Path>()?);
      } else if meta.path.is_ident("attached") {
        attached.push(parse_attached(&meta)?);
      } else if meta.path.is_ident("markup_extension") {
        markup_extension = Some(parse_markup_extension(&meta)?);
      } else {
        return Err(unknown_key(
          &meta,
          "`namespace`, `content`, `whitespace`, `crate`, `attached` or `markup_extension`",
        ));
      }
      Ok(())
    })?;
  }
  let namespace = namespace.ok_or_else(|| {
    Error::new_spanned(
      &input.ident,
      "missing the XML namespace: add `#[xaml(namespace = \"...\")]` to the struct",
    )
  })?;
  Ok(TypeAttributes {
    namespace,
    content,
    whitespace,
    crate_path,
    attached,
    markup_extension,
  })
}

/// Reads `markup_extension` or `markup_extension(positional(key, fallback))`, returning the
/// properties set by position.
fn parse_markup_extension(markup_extension: &ParseNestedMeta) -> syn::Result<Vec<Ident>> {
  let mut positional = Vec::new();
  if markup_extension.input.peek(syn::token::Paren) {
    markup_extension.parse_nested_meta(|meta| {
      if !meta.path.is_ident("positional") {
        return Err(unknown_key(&meta, "`positional`"));
      }
      meta.parse_nested_meta(|argument| {
        positional.push(argument.path.require_ident()?.clone());
        Ok(())
      })
    })?;
  }
  Ok(positional)
}

/// Reads `attached(name = "Row", ty = usize, default = 0, affects = layout, target = children)`.
fn parse_attached(attached: &ParseNestedMeta) -> syn::Result<Attached> {
  let mut name = None;
  let mut ty = None;
  let mut default = None;
  let mut affects = Affects::default();
  let mut target = None;
  attached.parse_nested_meta(|meta| {
    if meta.path.is_ident("name") {
      name = Some(meta.value()?.parse::<LitStr>()?);
    } else if meta.path.is_ident("ty") {
      ty = Some(meta.value()?.parse::<Type>()?);
    } else if meta.path.is_ident("default") {
      default = Some(meta.value()?.parse::<Expr>()?);
    } else if meta.path.is_ident("affects") {
      affects = parse_affects(meta.value()?)?;
    } else if meta.path.is_ident("target") {
      let value: Ident = meta.value()?.parse()?;
      target = Some(match value.to_string().as_str() {
        "children" => Target::Children,
        "any" => Target::Any,
        _ => return Err(Error::new_spanned(value, "expected `children` or `any`")),
      });
    } else if meta.path.is_ident("inherits") {
      return Err(meta.error("attached properties can't inherit yet"));
    } else {
      return Err(unknown_key(&meta, "`name`, `ty`, `default`, `affects` or `target`"));
    }
    Ok(())
  })?;
  let missing = |key: &str| attached.error(format_args!("`attached(...)` needs `{key} = ...`"));
  Ok(Attached {
    name: name.ok_or_else(|| missing("name"))?,
    ty: ty.ok_or_else(|| missing("ty"))?,
    default,
    affects,
    target: target.ok_or_else(|| missing("target"))?,
  })
}

/// Reads a field's `#[xaml]` or `#[xaml(...)]` attribute, or returns `None` if it has none.
pub fn parse_field_attributes(field: &Field) -> syn::Result<Option<FieldAttributes>> {
  let mut found = None;
  for attribute in field.attrs.iter().filter(|attribute| attribute.path().is_ident("xaml")) {
    let attributes = found.get_or_insert_with(FieldAttributes::default);
    if matches!(attribute.meta, Meta::Path(_)) {
      continue;
    }
    attribute.parse_nested_meta(|meta| {
      if meta.path.is_ident("default") {
        attributes.default = Some(meta.value()?.parse::<Expr>()?);
      } else if meta.path.is_ident("key") {
        attributes.key = Some(meta.value()?.parse::<Path>()?);
      } else if meta.path.is_ident("affects") {
        attributes.affects = Some(parse_affects(meta.value()?)?);
      } else if meta.path.is_ident("inherits") {
        attributes.inherits = Some(meta.path.get_ident().map_or_else(Span::call_site, Ident::span));
      } else if meta.path.is_ident("name") {
        attributes.name = Some(meta.value()?.parse::<LitStr>()?);
      } else if meta.path.is_ident("flatten") {
        attributes.flatten = Some(meta.path.get_ident().map_or_else(Span::call_site, Ident::span));
      } else {
        return Err(unknown_key(
          &meta,
          "`default`, `key`, `affects`, `inherits`, `name` or `flatten`",
        ));
      }
      Ok(())
    })?;
  }
  if let Some(attributes) = &found
    && attributes.flatten.is_some()
    && (attributes.default.is_some()
      || attributes.key.is_some()
      || attributes.affects.is_some()
      || attributes.inherits.is_some()
      || attributes.name.is_some())
  {
    return Err(Error::new_spanned(
      field,
      "`flatten` takes no other settings: the group's properties carry their own",
    ));
  }
  if let Some(attributes) = &found
    && let Some(key) = &attributes.key
    && (attributes.affects.is_some() || attributes.inherits.is_some())
  {
    return Err(Error::new_spanned(
      key,
      "a shared `key` already holds `affects` and `inherits`; set them on the key instead",
    ));
  }
  Ok(found)
}

fn parse_affects(input: syn::parse::ParseStream) -> syn::Result<Affects> {
  let mut affects = Affects::default();
  loop {
    let stage: Ident = input.parse()?;
    match stage.to_string().as_str() {
      "layout" => affects.layout = true,
      "render" => affects.render = true,
      _ => return Err(Error::new_spanned(stage, "expected `layout` or `render`")),
    }
    if !input.peek(Token![|]) {
      return Ok(affects);
    }
    input.parse::<Token![|]>()?;
  }
}

fn unknown_key(meta: &ParseNestedMeta, expected: &str) -> Error {
  let key = meta.path.get_ident().map_or_else(|| "?".to_owned(), ToString::to_string);
  meta.error(format_args!("unknown `xaml` key `{key}`; expected {expected}"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;
  use syn::parse_quote;

  /// Returns the first field of `input`.
  fn first_field(input: &DeriveInput) -> &Field {
    let syn::Data::Struct(data) = &input.data else {
      unreachable!("tests pass structs")
    };
    data.fields.iter().next().expect("the struct has a field")
  }

  // The struct attributes are read into plain values.
  #[test]
  fn type_attributes_are_parsed() {
    let input: DeriveInput = parse_quote! {
      #[xaml(namespace = "https://artic.rs/ui", content = child, whitespace = "significant")]
      struct Border { child: u8 }
    };

    let attributes = parse_type_attributes(&input).expect("valid attributes");

    assert_eq!(attributes.namespace.value(), "https://artic.rs/ui");
    assert_eq!(
      attributes.content.map(|content| content.to_string()),
      Some("child".to_owned())
    );
    assert_eq!(attributes.whitespace, Whitespace::Significant);
  }

  // `crate` names the path generated code uses to reach artic.
  #[test]
  fn crate_path_is_parsed() {
    let input: DeriveInput = parse_quote! {
      #[xaml(namespace = "https://artic.rs/ui", crate = "::my_artic")]
      struct Border;
    };

    let attributes = parse_type_attributes(&input).expect("valid attributes");

    let path = attributes.crate_path.expect("crate is set");
    assert_eq!(quote::quote!(#path).to_string(), ":: my_artic");
  }

  // An `attached(...)` entry is read with its type, default, stages and target.
  #[test]
  fn attached_is_parsed() {
    let input: DeriveInput = parse_quote! {
      #[xaml(namespace = "https://artic.rs/ui")]
      #[xaml(attached(name = "Row", ty = usize, default = 1, affects = layout, target = children))]
      #[xaml(attached(name = "Label", ty = String, target = any))]
      struct Grid;
    };

    let attributes = parse_type_attributes(&input).expect("valid attributes");

    let [row, label] = attributes.attached.as_slice() else {
      panic!("two attached properties");
    };
    assert_eq!(row.name.value(), "Row");
    let ty = &row.ty;
    assert_eq!(quote::quote!(#ty).to_string(), "usize");
    assert!(row.default.is_some());
    assert_eq!(
      row.affects,
      Affects {
        layout: true,
        render: false
      }
    );
    assert_eq!((row.target, label.target), (Target::Children, Target::Any));
    assert!(label.default.is_none());
  }

  // `attached(...)` needs a name, a type and a target, rejects `inherits`, and knows its targets.
  #[test]
  fn attached_errors() {
    let message = |attached: proc_macro2::TokenStream| {
      let input: DeriveInput = parse_quote! {
        #[xaml(namespace = "https://artic.rs/ui", attached(#attached))]
        struct Grid;
      };
      parse_type_attributes(&input).expect_err("invalid attached").to_string()
    };

    assert_eq!(
      message(quote::quote!(ty = u8, target = any)),
      "`attached(...)` needs `name = ...`"
    );
    assert_eq!(
      message(quote::quote!(name = "Row", target = any)),
      "`attached(...)` needs `ty = ...`"
    );
    assert_eq!(
      message(quote::quote!(name = "Row", ty = u8)),
      "`attached(...)` needs `target = ...`"
    );
    assert_eq!(
      message(quote::quote!(name = "Row", ty = u8, target = any, inherits)),
      "attached properties can't inherit yet"
    );
    assert_eq!(
      message(quote::quote!(name = "Row", ty = u8, target = parent)),
      "expected `children` or `any`"
    );
  }

  // `markup_extension` marks a markup extension, with the properties set by position in order.
  #[test]
  fn markup_extension_is_parsed() {
    let positional = |input: DeriveInput| {
      let attributes = parse_type_attributes(&input).expect("valid attributes");
      attributes
        .markup_extension
        .map(|names| names.iter().map(ToString::to_string).collect::<Vec<_>>())
    };

    let plain: DeriveInput = parse_quote! { #[xaml(namespace = "u")] struct Border; };
    let bare: DeriveInput = parse_quote! { #[xaml(namespace = "u", markup_extension)] struct Null; };
    let listed: DeriveInput = parse_quote! {
      #[xaml(namespace = "u", markup_extension(positional(key, fallback)))]
      struct Localize;
    };

    assert_eq!(positional(plain), None);
    assert_eq!(positional(bare), Some(Vec::new()));
    assert_eq!(positional(listed), Some(vec!["key".to_owned(), "fallback".to_owned()]));
  }

  // A struct without a namespace is rejected.
  #[test]
  fn missing_namespace_is_an_error() {
    let input: DeriveInput = parse_quote! { struct Border; };

    let error = parse_type_attributes(&input).expect_err("no namespace");

    assert!(error.to_string().starts_with("missing the XML namespace"));
  }

  // A field's `affects` takes `layout`, `render` or both joined with `|`.
  #[test]
  fn affects_lists_stages() {
    let input: DeriveInput = parse_quote! {
      struct Text { #[xaml(affects = layout | render, inherits, default = 12.0)] size: u8 }
    };

    let attributes = parse_field_attributes(first_field(&input))
      .expect("valid attributes")
      .expect("has #[xaml]");

    assert_eq!(
      attributes.affects,
      Some(Affects {
        layout: true,
        render: true
      })
    );
    assert!(attributes.inherits.is_some() && attributes.default.is_some());
  }

  // A bare `#[xaml]` marks the field with no settings, and no attribute means not a XAML field.
  #[test]
  fn bare_and_missing_field_attributes() {
    let bare: DeriveInput = parse_quote! { struct A { #[xaml] value: u8 } };
    let missing: DeriveInput = parse_quote! { struct A { value: u8 } };

    assert!(parse_field_attributes(first_field(&bare)).expect("valid").is_some());
    assert!(parse_field_attributes(first_field(&missing)).expect("valid").is_none());
  }

  // A shared key can't be combined with `affects` or `inherits`, which the key already holds.
  #[test]
  fn key_with_affects_is_an_error() {
    let input: DeriveInput = parse_quote! {
      struct Text { #[xaml(key = FONT_SIZE, affects = layout)] size: u8 }
    };

    let error = parse_field_attributes(first_field(&input)).expect_err("conflict");

    assert!(error.to_string().starts_with("a shared `key` already holds"));
  }
}
