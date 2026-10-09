//! What `xmlns` declarations bind prefixes to.

use crate::infoset::XamlNamespace;

/// The namespace an `xmlns` value names.
pub(super) fn classify(uri: &str) -> XamlNamespace {
  if uri == "https://artic.rs/markup" {
    XamlNamespace::Markup
  } else if let Some(path) = uri.strip_prefix("using:") {
    XamlNamespace::Using(path.to_owned())
  } else {
    XamlNamespace::Uri(uri.to_owned())
  }
}
