//! Derive and attribute macros for artic types, view-models and views.

mod crate_root;
mod xaml_enum;
mod xaml_type;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Describes a struct to the XAML registry, so markup can name it.
///
/// # Struct attributes
///
/// - `namespace = "..."` (required): the XML namespace the type's name belongs to.
/// - `content = field`: the `#[xaml]` field that receives content written inside the element.
/// - `whitespace = "significant"`: keep whitespace in text content (the default collapses it).
/// - `crate = "path"`: where generated code finds artic, if neither `artic` nor `artic-registry`
///   is a direct dependency under its usual name.
///
/// # Fields
///
/// Every `Property<T>` and `Event<A>` field must be marked `#[xaml]`:
///
/// - `Property<T>` is a XAML property. `T`, or `X` in `Option<X>`, must implement `FromStr` or
///   `XamlType`, or be a `NodeId` child slot. Its settings: `default = expr` for the `Default`
///   level (otherwise `T::default()`), `affects = layout | render` and `inherits` for its generated
///   key, `key = STATIC` to share a `PropertyKey` with other types instead (inheritance across
///   types needs it), and `name = "..."` to rename it (the field name in PascalCase otherwise).
/// - `Event<A>` is a XAML event named like a property.
/// - Any other `#[xaml]` field is a collection of child elements and must implement
///   `XamlCollection`.
///
/// Unmarked fields are created with their own `Default`.
///
/// # Generated code
///
/// The derive implements `Default`, `Object` (with `visit_properties`) and `XamlType`, writes the
/// type's `TypeInfo` with its property and event descriptors, and adds it to `XAML_TYPES`. So the
/// struct derives `Debug` but not `Default`, and a crate's types only reach the registry if the
/// crate is linked (see `XAML_TYPES`).
#[proc_macro_derive(XamlType, attributes(xaml))]
pub fn derive_xaml_type(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  xaml_type::expand(&input)
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}

/// Parses a fieldless enum from its variant names, ignoring ASCII case, so markup can set it from
/// text: `Orientation="horizontal"` becomes `Orientation::Horizontal`.
///
/// It implements `FromStr` with `Err = ParseEnumError`, whose message lists every valid name. Every
/// variant must be a unit variant, and no two names may differ only in case.
#[proc_macro_derive(XamlEnum)]
pub fn derive_xaml_enum(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  xaml_enum::expand(&input)
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}
