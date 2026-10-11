//! Derive and attribute macros for artic types, view-models and views.

mod crate_root;
mod xaml_enum;
mod xaml_properties;
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
/// - `attached(name = "Row", ty = usize, default = 0, affects = layout, target = children)`, once
///   per attached property the type owns: a property set on other elements as `Grid.Row`. `name`,
///   `ty` and `target` are required; `target = children` allows it only on the logical children
///   of an element of this type, `target = any` anywhere. The value type is checked like a
///   property's. Attached properties can't `inherit` yet.
/// - `markup_extension`, or `markup_extension(positional(key, ...))`: the type is a markup
///   extension like `{Localize greeting}`. Every property is a named argument; `positional` lists,
///   in order, the properties also set by position.
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
/// - `#[xaml(flatten)]` on a field whose type derives `XamlProperties`: its properties are
///   listed and visited as the type's own. It takes no other settings.
/// - Any other `#[xaml]` field is a collection of child elements and must implement
///   `XamlCollection`.
///
/// Unmarked fields are created with their own `Default`.
///
/// # Generated code
///
/// The derive implements `Default`, `Object` (with `visit_properties`) and `XamlType`, writes the
/// type's `TypeInfo` with its property, event, attached property and markup extension descriptors,
/// and adds it to `XAML_TYPES`. So the struct derives `Debug` but not `Default`, and a crate's types
/// only reach the registry if the crate is linked (see `XAML_TYPES`).
///
/// For each attached property it also writes, on the owner, the key constant and the helper pair
/// from the builder design, which read and write the value on any node:
///
/// ```ignore
/// pub const ROW: &'static AttachedKey<usize>;
/// pub fn set_row(tree: &mut UiTree, node: impl Into<NodeId>, row: usize);
/// pub fn row(tree: &UiTree, node: impl Into<NodeId>) -> usize;
/// ```
///
/// A helper named like one of the type's properties is an error: rename one with `name = ...`.
#[proc_macro_derive(XamlType, attributes(xaml))]
pub fn derive_xaml_type(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  xaml_type::expand(&input)
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}

/// Makes a struct a property group: a set of properties several XAML types share by embedding it
/// in a field marked `#[xaml(flatten)]`, as controls embed `LayoutProperties`.
///
/// Fields follow `XamlType`'s rules for `Property` fields; a group holds no events or
/// collections, and takes no attributes on the struct. The derive implements `Default` and
/// `PropertyGroup`. A group's keys are its own, so every type that flattens it shares them, and
/// an inheriting property inherits across those types.
#[proc_macro_derive(XamlProperties, attributes(xaml))]
pub fn derive_xaml_properties(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  xaml_properties::expand(&input)
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
