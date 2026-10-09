// Integration tests for `parse`, using only the public API, as the back ends would.

use artic_xaml::diagnostics::SourceMap;
use artic_xaml::infoset::XamlNode;
use artic_xaml::parse;

// The example view from the technical spec (§4) parses into the reviewed infoset.
#[test]
fn spec_example_view() {
  let mut sources = SourceMap::new();
  let file = sources.add("login_view.xaml", include_str!("fixtures/login_view.xaml"));

  let document = parse(&sources, file).unwrap_or_else(|diagnostics| panic!("{}", diagnostics.render(&sources)));

  insta::assert_debug_snapshot!(document);
}

// Every markup extension the spec and design docs show parses into an extension of that name.
#[test]
fn spec_markup_extensions() {
  let examples = [
    "{Binding}",
    "{Binding Id}",
    "{Binding Save}",
    "{Binding SignIn}",
    "{Binding Email, Mode=TwoWay}",
    "{Binding Path=Name, Mode=TwoWay}",
    "{Binding RelativeSource={RelativeSource TemplatedParent}}",
    "{RelativeSource TemplatedParent}",
    "{TemplateBinding Prop}",
    "{StaticResource}",
    "{StaticResource Accent}",
    "{StaticResource Gap}",
    "{StaticResource Key}",
    "{DynamicResource}",
    "{DynamicResource AccentHover}",
    "{DynamicResource Key}",
    "{Localize Key}",
    "{x:Null}",
    "{x:Static}",
    "{x:Static local::CONSTANTS::MAX}",
    "{x:Type Button}",
  ];
  for example in examples {
    let mut sources = SourceMap::new();
    let file = sources.add(
      "example.xaml",
      format!(r#"<Button xmlns="https://artic.rs/ui" xmlns:x="https://artic.rs/markup" Tag="{example}"/>"#),
    );

    let document = parse(&sources, file).unwrap_or_else(|diagnostics| panic!("{}", diagnostics.render(&sources)));

    let XamlNode::Object { members, .. } = &document.root else {
      unreachable!("the root is an object");
    };
    let XamlNode::MarkupExtension { ty, .. } = &members[0].value[0] else {
      panic!("{example} isn't a markup extension");
    };
    let name = example[1..].split([' ', '}']).next().expect("a name");
    assert_eq!(ty.name, name.rsplit(':').next().expect("a local name"), "in {example}");
  }
}
