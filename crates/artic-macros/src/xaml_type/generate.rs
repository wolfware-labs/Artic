use crate::crate_root::crate_root;
use crate::xaml_type::attributes::{Affects, Whitespace};
use crate::xaml_type::fields::{Analysis, Role, XamlField, xaml_name};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::Ident;
use syn::spanned::Spanned;

/// Writes the code for one derived struct: its property keys, `Default`, `Object` and `XamlType`
/// impls, its registry descriptors and entry, and the checks on its field types.
pub fn generate(analysis: &Analysis) -> syn::Result<TokenStream> {
  let root = crate_root(analysis.attributes.crate_path.as_ref(), analysis.ident)?;
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

  let keys = properties.iter().map(|field| key_static(field));
  let default_fields = analysis.xaml_fields.iter().map(|field| default_field(field));
  let other_fields = &analysis.other_fields;
  let visits = properties.iter().map(|field| {
    let field_ident = field.ident;
    quote!(visit(&self.#field_ident);)
  });
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
  let value_checks = properties.iter().map(|field| {
    let Role::Property { value, .. } = field.role else {
      unreachable!("filtered to properties")
    };
    quote_spanned!(value.span()=>
      (&&__artic::ValueCheck::<#value>(__artic::PhantomData)).value_must_implement_fromstr_or_derive_xamltype();
    )
  });
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
          Self {
            #(#default_fields,)*
            #(#other_fields: ::core::default::Default::default(),)*
          }
        }
      }

      impl __artic::Object for #ident {
        fn visit_properties(&self, visit: &mut dyn ::core::ops::FnMut(&dyn __artic::AnyProperty)) {
          #(#visits)*
        }
      }

      static __PROPERTIES: [__artic::PropertyInfo; #property_count] = [#(#property_infos),*];
      static __EVENTS: [__artic::EventInfo; #event_count] = [#(#event_infos),*];

      fn __new(arena: &mut __artic::TreeArena) -> __artic::NodeId {
        arena.alloc(::std::boxed::Box::new(<#ident as ::core::default::Default>::default()))
      }

      static __TYPE_INFO: __artic::TypeInfo = __artic::TypeInfo {
        name: ::core::stringify!(#ident),
        namespace: #namespace,
        rust_path: ::core::concat!(::core::module_path!(), "::", ::core::stringify!(#ident)),
        new: __new,
        content: #content,
        is_collection: #is_collection,
        properties: &__PROPERTIES,
        events: &__EVENTS,
        attached_properties: &[],
        markup_extension: ::core::option::Option::None,
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
      }
    };
  })
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
fn key_static(field: &XamlField) -> TokenStream {
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
fn default_field(field: &XamlField) -> TokenStream {
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
    Role::Event { .. } | Role::Collection => quote!(#field_ident: ::core::default::Default::default()),
  }
}

/// The `PropertyInfo` for one property, with its metadata read from the key.
fn property_info(field: &XamlField) -> TokenStream {
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
