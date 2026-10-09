//! Derive and attribute macros for artic types, view-models and views.

mod xaml_type;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Describes a struct to the XAML registry, so markup can name it.
///
/// The struct needs `#[xaml(namespace = "...")]`. The rest of the attributes and the generated code
/// are documented with the derive's full behavior.
#[proc_macro_derive(XamlType, attributes(xaml))]
pub fn derive_xaml_type(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  xaml_type::expand(&input)
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}
