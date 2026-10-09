//! Turning XAML text into the infoset.

mod builder;
#[cfg(test)]
mod tests;
mod text;

use crate::diagnostics::{Diagnostics, SourceId, SourceMap};
use crate::infoset::XamlDocument;
use builder::Builder;

/// Parses the XAML file `file` of `sources` into its infoset.
///
/// # Errors
///
/// Returns every problem found: malformed XML stops parsing at the first error, while misplaced
/// attributes and elements are all collected before returning.
///
/// # Examples
///
/// ```
/// use artic_xaml::diagnostics::SourceMap;
/// use artic_xaml::infoset::{MemberKey, XamlNode};
/// use artic_xaml::parse;
///
/// let mut sources = SourceMap::new();
/// let file = sources.add("view.xaml", r#"<Button Width="100">OK</Button>"#);
/// let document = parse(&sources, file).expect("valid XAML");
/// let XamlNode::Object { ty, members, .. } = &document.root else { unreachable!() };
/// assert_eq!(ty.name, "Button");
/// assert_eq!(members[0].key, MemberKey::Property("Width".to_owned()));
/// assert_eq!(members[1].key, MemberKey::Content);
/// ```
pub fn parse(sources: &SourceMap, file: SourceId) -> Result<XamlDocument, Diagnostics> {
  Builder::new(file, sources.get(file)).run()
}
