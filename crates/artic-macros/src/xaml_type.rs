mod attributes;

use attributes::{parse_field_attributes, parse_type_attributes};
use proc_macro2::TokenStream;
use syn::{Data, DeriveInput, Error, Fields};

pub fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
  let Data::Struct(data) = &input.data else {
    return Err(Error::new_spanned(
      &input.ident,
      "`XamlType` can only be derived for a struct",
    ));
  };
  let Fields::Named(fields) = &data.fields else {
    if matches!(data.fields, Fields::Unit) {
      parse_type_attributes(input)?;
      return Ok(TokenStream::new());
    }
    return Err(Error::new_spanned(
      &data.fields,
      "`XamlType` needs a struct with named fields",
    ));
  };
  parse_type_attributes(input)?;
  for field in &fields.named {
    parse_field_attributes(field)?;
  }
  Ok(TokenStream::new())
}
