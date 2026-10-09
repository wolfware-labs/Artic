use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{Error, Ident, Path};

/// The path generated code uses to reach artic: `override_path` if given, or whichever of `artic`
/// and `artic-registry` the calling crate depends on. Generated code then names
/// `<root>::__private::...`. The error points at `target`.
pub fn crate_root(override_path: Option<&Path>, target: &impl ToTokens) -> syn::Result<TokenStream> {
  if let Some(path) = override_path {
    return Ok(quote!(#path));
  }
  for (package, library) in [("artic", "artic"), ("artic-registry", "artic_registry")] {
    match crate_name(package) {
      Ok(FoundCrate::Name(name)) => {
        let name = Ident::new(&name, Span::call_site());
        return Ok(quote!(::#name));
      }
      Ok(FoundCrate::Itself) => {
        let name = Ident::new(library, Span::call_site());
        return Ok(quote!(::#name));
      }
      Err(_) => {}
    }
  }
  Err(Error::new_spanned(
    target,
    "this derive needs `artic` (or `artic-registry`) in this crate's dependencies",
  ))
}
