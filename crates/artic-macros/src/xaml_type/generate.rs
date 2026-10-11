use crate::crate_root::{REGISTRY, crate_root};
use crate::xaml_type::attributes::{Affects, Attached, Target, Whitespace};
use crate::xaml_type::fields::{Analysis, Role, XamlField, generic_argument, snake_case, xaml_name};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::Ident;
use syn::spanned::Spanned;

/// Writes the code for one derived struct: its property keys, `Default`, `Object` and `XamlType`
/// impls, its registry descriptors and entry, and the checks on its field types.
pub fn generate(analysis: &Analysis) -> syn::Result<TokenStream> {
  let root = crate_root(analysis.attributes.crate_path.as_ref(), analysis.ident, REGISTRY)?;
  let ident = analysis.ident;
  let properties: Vec<&XamlField> = analysis.xaml_fields.iter().filter(|f| is_property(f)).collect();
  let events: Vec<&XamlField> = analysis
    .xaml_fields
    .iter()
    .filter(|f| matches!(f.role, Role::Event { .. }))
    .collect();
  let collections: Vec<&XamlField> = analysis
    .xaml_fields
    .iter()
    .filter(|f| matches!(f.role, Role::Collection))
    .collect();
  let groups: Vec<&XamlField> = analysis
    .xaml_fields
    .iter()
    .filter(|f| matches!(f.role, Role::Group))
    .collect();

  let keys = properties.iter().map(|field| key_static(field));
  let default_fields = analysis.xaml_fields.iter().map(|field| default_field(field));
  let other_fields = &analysis.other_fields;
  let visits = properties.iter().map(|field| {
    let field_ident = field.ident;
    quote!(visit(&self.#field_ident);)
  });
  let group_visits = groups.iter().map(|field| {
    let field_ident = field.ident;
    quote!(__artic::PropertyGroup::visit_properties(&self.#field_ident, visit);)
  });
  let group_infos = groups.iter().map(|field| {
    let ty = field.ty;
    let field_name = field.ident.to_string();
    quote!(__artic::PropertyGroupInfo { field: #field_name, properties: <#ty as __artic::PropertyGroup>::PROPERTIES })
  });
  let group_checks = groups.iter().map(|field| {
    let ty = field.ty;
    quote_spanned!(ty.span()=> __artic::assert_property_group::<#ty>();)
  });
  let group_count = groups.len();
  let property_infos = properties.iter().map(|field| property_info(field));
  let event_infos = events.iter().map(|field| {
    let name = xaml_name(field);
    let Role::Event { args } = field.role else {
      unreachable!("filtered to events")
    };
    quote!(__artic::EventInfo { name: #name, args_type: ::core::stringify!(#args) })
  });
  let property_count = properties.len();
  let event_count = events.len();
  let value_checks = properties.iter().map(|field| value_check(field));
  let collection_checks = collections.iter().map(|field| {
    let ty = field.ty;
    quote_spanned!(ty.span()=> __artic::assert_collection::<#ty>();)
  });
  let namespace = &analysis.attributes.namespace;
  let content = match &analysis.attributes.content {
    Some(content) => {
      let content = content.to_string();
      quote!(::core::option::Option::Some(#content))
    }
    None => quote!(::core::option::Option::None),
  };
  let is_collection = !collections.is_empty();
  let attached = &analysis.attributes.attached;
  let attached_keys = attached.iter().map(attached_key);
  let attached_helpers = attached.iter().map(|attached| attached_helpers(ident, attached));
  let attached_infos = attached.iter().map(attached_info);
  let attached_count = attached.len();
  let markup_extension = markup_extension(analysis, &properties);
  let markup_extension_info = if analysis.attributes.markup_extension.is_some() {
    quote!(::core::option::Option::Some(&__MARKUP_EXTENSION))
  } else {
    quote!(::core::option::Option::None)
  };
  let attached_checks = attached.iter().map(|attached| {
    let ty = &attached.ty;
    let value = generic_argument(ty, "Option").unwrap_or(ty);
    quote_spanned!(ty.span()=>
      (&&__artic::ValueCheck::<#value>(__artic::PhantomData)).value_must_implement_fromstr_or_derive_xamltype();
    )
  });
  let after_default = &analysis.after_default;
  let whitespace = match analysis.attributes.whitespace {
    Whitespace::Collapse => quote!(__artic::Whitespace::Collapse),
    Whitespace::Significant => quote!(__artic::Whitespace::Significant),
  };

  Ok(quote! {
    const _: () = {
      use #root::__private as __artic;

      #(#keys)*

      impl ::core::default::Default for #ident {
        fn default() -> Self {
          let value = Self {
            #(#default_fields,)*
            #(#other_fields: ::core::default::Default::default(),)*
          };
          #after_default
          value
        }
      }

      impl __artic::Object for #ident {
        fn visit_properties(&self, visit: &mut dyn ::core::ops::FnMut(&dyn __artic::AnyProperty)) {
          #(#visits)*
          #(#group_visits)*
        }
      }

      static __PROPERTIES: [__artic::PropertyInfo; #property_count] = [#(#property_infos),*];
      static __EVENTS: [__artic::EventInfo; #event_count] = [#(#event_infos),*];
      static __ATTACHED: [__artic::AttachedPropertyInfo; #attached_count] = [#(#attached_infos),*];
      static __GROUPS: [__artic::PropertyGroupInfo; #group_count] = [#(#group_infos),*];

      #(#attached_keys)*

      #markup_extension

      impl #ident {
        #(#attached_helpers)*
      }

      fn __new(tree: &mut __artic::UiTree) -> __artic::NodeId {
        tree.alloc(::std::boxed::Box::new(<#ident as ::core::default::Default>::default()))
      }

      static __TYPE_INFO: __artic::TypeInfo = __artic::TypeInfo {
        name: ::core::stringify!(#ident),
        namespace: #namespace,
        rust_path: ::core::concat!(::core::module_path!(), "::", ::core::stringify!(#ident)),
        new: __new,
        content: #content,
        is_collection: #is_collection,
        properties: &__PROPERTIES,
        groups: &__GROUPS,
        events: &__EVENTS,
        attached_properties: &__ATTACHED,
        markup_extension: #markup_extension_info,
        whitespace: #whitespace,
      };

      impl __artic::XamlType for #ident {
        const TYPE_INFO: &'static __artic::TypeInfo = &__TYPE_INFO;
      }

      __artic::register_type!(__TYPE_INFO);

      #[allow(dead_code, clippy::needless_borrow)]
      fn __checks() {
        use __artic::{ViaChild as _, ViaFromStr as _, ViaXamlType as _};
        #(#value_checks)*
        #(#collection_checks)*
        #(#group_checks)*
        #(#attached_checks)*
      }
    };
  })
}

/// The `MarkupExtensionInfo` static of a markup extension, or nothing for another type. Every
/// property is a named argument, and `positional(...)` lists those also set by position.
fn markup_extension(analysis: &Analysis, properties: &[&XamlField]) -> TokenStream {
  let Some(positional) = &analysis.attributes.markup_extension else {
    return TokenStream::new();
  };
  let by_position: Vec<&XamlField> = positional
    .iter()
    .map(|argument| {
      *properties
        .iter()
        .find(|field| field.ident == argument)
        .expect("checked to name a property")
    })
    .collect();
  let positional = by_position.iter().map(|field| arg_info(field));
  let named = properties.iter().map(|field| arg_info(field));
  let (positional_count, named_count) = (by_position.len(), properties.len());
  quote! {
    static __POSITIONAL: [__artic::ArgInfo; #positional_count] = [#(#positional),*];
    static __NAMED: [__artic::ArgInfo; #named_count] = [#(#named),*];
    static __MARKUP_EXTENSION: __artic::MarkupExtensionInfo = __artic::MarkupExtensionInfo {
      positional: &__POSITIONAL,
      named: &__NAMED,
    };
  }
}

/// The `ArgInfo` of a property used as a markup extension argument: its name and value type, as in
/// its `PropertyInfo`.
fn arg_info(field: &XamlField) -> TokenStream {
  let Role::Property { inner, .. } = field.role else {
    unreachable!("properties only")
  };
  let key = key_path(field);
  quote!(__artic::ArgInfo { name: #key.name(), value_type: ::core::stringify!(#inner) })
}

/// The static an attached property's key lives in, such as `__ATTACHED_COLUMN_SPAN`.
fn attached_static(attached: &Attached) -> Ident {
  format_ident!("__ATTACHED_{}", snake_case(&attached.name.value()).to_uppercase())
}

/// The `static AttachedKey` of one attached property.
fn attached_key(attached: &Attached) -> TokenStream {
  let key = attached_static(attached);
  let Attached {
    name,
    ty,
    default,
    affects,
    ..
  } = attached;
  let default = match default {
    Some(expression) => quote!(#expression),
    None => quote_spanned!(ty.span()=> <#ty as ::core::default::Default>::default()),
  };
  let affects = affects_tokens(*affects);
  quote! {
    static #key: __artic::AttachedKey<#ty> = __artic::AttachedKey::new(#name, #affects, || #default);
  }
}

/// The key constant and the `set_<name>` and `<name>` helpers of one attached property.
fn attached_helpers(owner: &Ident, attached: &Attached) -> TokenStream {
  let key = attached_static(attached);
  let ty = &attached.ty;
  let name = attached.name.value();
  let snake = snake_case(&name);
  let constant = format_ident!("{}", snake.to_uppercase());
  let getter = format_ident!("{snake}");
  let setter = format_ident!("set_{snake}");
  let value = format_ident!("{snake}");
  let property = format!("`{owner}.{name}`");
  let constant_doc = format!("The {property} attached property.");
  let setter_doc = format!("Sets {property} on `node`, at local precedence.");
  let getter_doc = format!("Returns {property} of `node`, or its default when nothing set it.");
  quote! {
    #[doc = #constant_doc]
    pub const #constant: &'static __artic::AttachedKey<#ty> = &#key;

    #[doc = #setter_doc]
    pub fn #setter(tree: &mut __artic::UiTree, node: impl ::core::convert::Into<__artic::NodeId>, #value: #ty) {
      tree.attached_property(node.into(), Self::#constant).set(#value);
    }

    #[doc = #getter_doc]
    #[must_use]
    pub fn #getter(tree: &__artic::UiTree, node: impl ::core::convert::Into<__artic::NodeId>) -> #ty {
      tree.attached(node.into(), Self::#constant)
    }
  }
}

/// The `AttachedPropertyInfo` of one attached property.
fn attached_info(attached: &Attached) -> TokenStream {
  let Attached { name, ty, target, .. } = attached;
  let target = match target {
    Target::Any => quote!(__artic::AttachedTarget::Any),
    Target::Children => quote!(__artic::AttachedTarget::ChildrenOfOwner),
  };
  quote! {
    __artic::AttachedPropertyInfo {
      name: #name,
      value_type: ::core::stringify!(#ty),
      target: #target,
    }
  }
}

/// The check that a property's value type can be written in markup, pointing at the type.
pub fn value_check(field: &XamlField) -> TokenStream {
  let Role::Property { value, .. } = field.role else {
    unreachable!("properties only")
  };
  quote_spanned!(value.span()=>
    (&&__artic::ValueCheck::<#value>(__artic::PhantomData)).value_must_implement_fromstr_or_derive_xamltype();
  )
}

fn is_property(field: &XamlField) -> bool {
  matches!(field.role, Role::Property { .. })
}

/// The path of the key a property uses: its shared `key`, or the key the derive generates.
fn key_path(field: &XamlField) -> TokenStream {
  match &field.attributes.key {
    Some(key) => quote!(#key),
    None => {
      let key = generated_key(field.ident);
      quote!(#key)
    }
  }
}

fn generated_key(field: &Ident) -> Ident {
  format_ident!("__KEY_{}", field.to_string().to_uppercase())
}

/// The `static PropertyKey` the derive writes for a property without a shared `key`.
pub fn key_static(field: &XamlField) -> TokenStream {
  if field.attributes.key.is_some() {
    return TokenStream::new();
  }
  let Role::Property { inner, .. } = field.role else {
    unreachable!("properties only")
  };
  let key = generated_key(field.ident);
  let name = xaml_name(field);
  let affects = affects_tokens(field.attributes.affects.unwrap_or_default());
  let inherits = field.attributes.inherits.is_some();
  quote! {
    static #key: __artic::PropertyKey<#inner> = __artic::PropertyKey::new(#name, #affects, #inherits);
  }
}

fn affects_tokens(affects: Affects) -> TokenStream {
  match (affects.layout, affects.render) {
    (false, false) => quote!(__artic::Affects::NONE),
    (true, false) => quote!(__artic::Affects::LAYOUT),
    (false, true) => quote!(__artic::Affects::RENDER),
    (true, true) => quote!(__artic::Affects::LAYOUT.union(__artic::Affects::RENDER)),
  }
}

/// How `Default` creates one `#[xaml]` field.
pub fn default_field(field: &XamlField) -> TokenStream {
  let field_ident = field.ident;
  match field.role {
    Role::Property { inner, .. } => {
      let key = key_path(field);
      let value = match &field.attributes.default {
        Some(expression) => quote!(#expression),
        None => quote_spanned!(inner.span()=> <#inner as ::core::default::Default>::default()),
      };
      quote!(#field_ident: __artic::Property::new(&#key, #value))
    }
    Role::Event { .. } | Role::Collection | Role::Group => quote!(#field_ident: ::core::default::Default::default()),
  }
}

/// The `PropertyInfo` for one property, with its metadata read from the key.
pub fn property_info(field: &XamlField) -> TokenStream {
  let Role::Property { inner, .. } = field.role else {
    unreachable!("properties only")
  };
  let key = key_path(field);
  let field_name = field.ident.to_string();
  quote! {
    __artic::PropertyInfo {
      name: #key.name(),
      field: #field_name,
      value_type: ::core::stringify!(#inner),
      affects: #key.affects(),
      inherits: #key.inherits(),
    }
  }
}
