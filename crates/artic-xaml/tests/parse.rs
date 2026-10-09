// Integration tests for `parse`, using only the public API, as the back ends would.

use artic_xaml::diagnostics::SourceMap;
use artic_xaml::parse;

// The example view from the technical spec (§4) parses into the reviewed infoset.
#[test]
fn spec_example_view() {
  let mut sources = SourceMap::new();
  let file = sources.add("login_view.xaml", include_str!("fixtures/login_view.xaml"));

  let document = parse(&sources, file).unwrap_or_else(|diagnostics| panic!("{}", diagnostics.render(&sources)));

  insta::assert_debug_snapshot!(document);
}
