use super::*;
use pretty_assertions::assert_eq;

/// Parses `text`, panicking on errors, and returns the root node.
fn root(text: &str) -> XamlNode {
  let mut sources = SourceMap::new();
  let file = sources.add("test.xaml", text);
  match parse(&sources, file) {
    Ok(document) => document.root,
    Err(diagnostics) => panic!("unexpected errors: {}", diagnostics.render(&sources)),
  }
}

/// Parses `text`, expecting errors, and returns each error's message with the source text it points
/// at.
fn errors(text: &str) -> Vec<(String, &str)> {
  let mut sources = SourceMap::new();
  let file = sources.add("test.xaml", text);
  let diagnostics = parse(&sources, file).expect_err("the document has errors");
  diagnostics
    .iter()
    .map(|diagnostic| (diagnostic.message.clone(), &text[diagnostic.span.start..diagnostic.span.end]))
    .collect()
}

/// The members of an object node.
fn members(node: &XamlNode) -> &[Member] {
  match node {
    XamlNode::Object { members, .. } => members,
    other => panic!("expected an object, found {other:?}"),
  }
}

/// The member keys of an object node, in order.
fn keys(node: &XamlNode) -> Vec<MemberKey> {
  members(node).iter().map(|member| member.key.clone()).collect()
}

/// The text of a value node.
fn text(node: &XamlNode) -> &str {
  match node {
    XamlNode::Value { text, .. } => text,
    other => panic!("expected a value, found {other:?}"),
  }
}

/// The type name of an object node.
fn type_name(node: &XamlNode) -> &str {
  match node {
    XamlNode::Object { ty, .. } => &ty.name,
    other => panic!("expected an object, found {other:?}"),
  }
}

/// The id of the file the helpers above parse: the first one added to a new map.
fn first_file() -> SourceId {
  SourceMap::new().add("test.xaml", "")
}

fn property(name: &str) -> MemberKey {
  MemberKey::Property(name.to_owned())
}

fn attached(prefix: Option<&str>, owner: &str, name: &str, span: Span) -> MemberKey {
  MemberKey::Attached {
    owner: TypeRef {
      prefix: prefix.map(str::to_owned),
      name: owner.to_owned(),
      span,
    },
    name: name.to_owned(),
  }
}

// An attribute becomes a property member holding its value.
#[test]
fn attributes_are_properties() {
  let root = root(r#"<Button Width="100" Content="OK"/>"#);

  assert_eq!(type_name(&root), "Button");
  assert_eq!(keys(&root), [property("Width"), property("Content")]);
  assert_eq!(text(&members(&root)[0].value[0]), "100");
}

// `xmlns` attributes become namespace declarations, not members.
#[test]
fn xmlns_attributes_are_namespace_declarations() {
  let root = root(r#"<Page xmlns="urn:ui" xmlns:x="urn:x"/>"#);

  let XamlNode::Object { namespaces, members, .. } = &root else {
    unreachable!()
  };
  let declared: Vec<_> = namespaces
    .iter()
    .map(|namespace| (namespace.prefix.as_deref(), namespace.uri.as_str()))
    .collect();
  assert_eq!(declared, [(None, "urn:ui"), (Some("x"), "urn:x")]);
  assert!(members.is_empty());
}

// `x:` attributes and `xml:space` are directives; an unknown `x:` name is kept as `Other`.
#[test]
fn x_attributes_are_directives() {
  let root = root(r#"<Button x:Name="ok" x:Key="k" x:Shared="false" xml:space="preserve"/>"#);

  assert_eq!(
    keys(&root),
    [
      MemberKey::Directive(Directive::Name),
      MemberKey::Directive(Directive::Key),
      MemberKey::Directive(Directive::Other("Shared".to_owned())),
      MemberKey::Directive(Directive::XmlSpace),
    ]
  );
}

// `Owner.Name` attributes are attached properties, unless the owner is the element's own type.
#[test]
fn dotted_attributes_are_attached_properties() {
  let source = r#"<Button Grid.Row="1" ui:Dock.Side="Left" Button.Width="2"/>"#;
  let root = root(source);

  let span = |owner: &str| {
    let start = source.find(owner).expect("owner in source");
    Span::new(first_file(), start, start + owner.len())
  };
  assert_eq!(
    keys(&root),
    [
      attached(None, "Grid", "Row", span("Grid")),
      attached(Some("ui"), "Dock", "Side", span("ui:Dock")),
      property("Width"),
    ]
  );
}

// `<Type.Name>` elements are property elements; any other owner makes them attached.
#[test]
fn property_elements() {
  let source = "<Button><Button.Content><Image/></Button.Content><Grid.Row>1</Grid.Row></Button>";
  let root = root(source);

  let start = source.find("Grid").expect("owner in source");
  let owner = Span::new(first_file(), start, start + "Grid".len());
  assert_eq!(keys(&root), [property("Content"), attached(None, "Grid", "Row", owner)]);
  assert_eq!(type_name(&members(&root)[0].value[0]), "Image");
  assert_eq!(text(&members(&root)[1].value[0]), "1");
}

// Text and child objects go, in document order, into one content member after the other members.
#[test]
fn content_is_one_member_in_document_order() {
  let root = root("<Panel>a<Image/>b<Panel.Tag>t</Panel.Tag></Panel>");

  assert_eq!(keys(&root), [property("Tag"), MemberKey::Content]);
  let content = &members(&root)[1].value;
  assert_eq!(content.len(), 3);
  assert_eq!(text(&content[0]), "a");
  assert_eq!(type_name(&content[1]), "Image");
  assert_eq!(text(&content[2]), "b");
}

// Entities are decoded, CDATA is kept verbatim, and adjacent text and CDATA merge into one value.
#[test]
fn text_and_cdata_merge_into_one_value() {
  let root = root("<Text>a &amp; <![CDATA[<b> &amp;]]> c</Text>");

  let content = &members(&root)[0].value;
  assert_eq!(content.len(), 1);
  assert_eq!(text(&content[0]), "a & <b> &amp; c");
}

// Whitespace between elements is kept as text: deciding what it means is the loader's job.
#[test]
fn whitespace_is_kept() {
  let root = root("<Panel>\n  <Image/>\n</Panel>");

  let content = &members(&root)[0].value;
  assert_eq!(text(&content[0]), "\n  ");
  assert_eq!(text(&content[2]), "\n");
}

// Attribute values are normalized as XML requires: whitespace characters become spaces.
#[test]
fn attribute_values_are_normalized() {
  let root = root("<Text Value=\"a\r\nb\tc &amp; &#10;\"/>");

  assert_eq!(text(&members(&root)[0].value[0]), "a b c & \n");
}

// Comments, the XML declaration and processing instructions are dropped.
#[test]
fn comments_and_declarations_are_dropped() {
  let root = root("<?xml version=\"1.0\"?>\n<!-- top --><Panel><!-- inside --><?pi data?></Panel>\n");

  assert_eq!(type_name(&root), "Panel");
  assert!(members(&root).is_empty());
}

// Every node and member points at the source text it came from.
#[test]
fn spans_point_at_source() {
  let source = r#"<ui:Button xmlns:ui="urn:ui" Width="100">OK<ui:Button.Tag/></ui:Button>"#;
  let root = root(source);
  let at = |span: Span| &source[span.start..span.end];

  let XamlNode::Object {
    ty,
    members,
    namespaces,
    span,
  } = &root
  else {
    unreachable!()
  };
  assert_eq!(at(*span), source);
  assert_eq!(at(ty.span), "ui:Button");
  assert_eq!(at(namespaces[0].span), r#"xmlns:ui="urn:ui""#);
  assert_eq!(at(members[0].span), "Width");
  assert_eq!(at(members[0].value[0].span()), "100");
  assert_eq!(at(members[1].span), "ui:Button.Tag");
  assert_eq!(at(members[2].span), "OK");
}

// Malformed XML stops parsing with one error at the position the tokenizer reports: for a bad
// attribute, where the attribute starts.
#[test]
fn malformed_xml_reports_span() {
  let source = "<Panel>\n  <Image Width=100/>\n</Panel>";

  let found = errors(source);

  assert_eq!(found.len(), 1);
  assert!(found[0].0.starts_with("malformed XML"), "{}", found[0].0);
  let offset = source.find(" Width").expect("attribute in source");
  let mut sources = SourceMap::new();
  let file = sources.add("test.xaml", source);
  let diagnostics = parse(&sources, file).expect_err("malformed");
  let span = diagnostics.iter().next().expect("one error").span;
  assert_eq!((span.start, span.end), (offset, offset));
}

// A wrong end tag points at itself and at the start tag it should close.
#[test]
fn mismatched_end_tag() {
  let source = "<Panel><Image></Panel>";
  let mut sources = SourceMap::new();
  let file = sources.add("test.xaml", source);

  let diagnostics = parse(&sources, file).expect_err("mismatched");

  let error = diagnostics.iter().next().expect("one error");
  assert_eq!(error.message, "expected `</Image>`, found `</Panel>`");
  assert_eq!(&source[error.span.start..error.span.end], "</Panel>");
  let opened = &error.labels[0];
  assert_eq!(&source[opened.span.start..opened.span.end], "Image");
  assert_eq!(opened.message, "opened here");
}

// An element still open at the end of the file is reported there, pointing back at its start tag.
#[test]
fn unclosed_element() {
  let source = "<Panel><Image/>";
  let mut sources = SourceMap::new();
  let file = sources.add("test.xaml", source);

  let diagnostics = parse(&sources, file).expect_err("unclosed");

  let error = diagnostics.iter().next().expect("one error");
  assert_eq!(error.message, "`<Panel>` is never closed");
  assert_eq!((error.span.start, error.span.end), (source.len(), source.len()));
  assert_eq!(&source[error.labels[0].span.start..error.labels[0].span.end], "Panel");
}

// The document needs exactly one root element, and it must be an object element.
#[test]
fn root_problems() {
  assert_eq!(
    errors("<!-- nothing -->"),
    [("expected one root element".to_owned(), "<!-- nothing -->")]
  );
  assert_eq!(
    errors("<A/><B/>"),
    [("expected one root element, found more content after it".to_owned(), "")]
  );
  assert_eq!(
    errors("<A.B/>"),
    [("the root element can't be a property element".to_owned(), "A.B")]
  );
}

// A DTD is rejected.
#[test]
fn dtd_is_an_error() {
  let found = errors("<!DOCTYPE Panel><Panel/>");

  assert_eq!(found, [("DTDs aren't supported in XAML".to_owned(), "<!DOCTYPE Panel>")]);
}

// A member set twice, by attribute or property element, is an error pointing at both.
#[test]
fn duplicate_members() {
  let found = errors(r#"<Button Width="1" Width="2" Grid.Row="1" Grid.Row="2"/>"#);
  let twice = "this member is set twice on the same element".to_owned();
  assert_eq!(found, [(twice.clone(), "Width"), (twice, "Grid.Row")]);

  let source = r#"<Button Tag="1"><Button.Tag>2</Button.Tag></Button>"#;
  let mut sources = SourceMap::new();
  let file = sources.add("test.xaml", source);
  let diagnostics = parse(&sources, file).expect_err("duplicate");
  let error = diagnostics.iter().next().expect("one error");
  assert_eq!(&source[error.span.start..error.span.end], "Button.Tag");
  assert_eq!(&source[error.labels[0].span.start..error.labels[0].span.end], "Tag");
}

// Misplaced attributes and elements are all reported, not only the first.
#[test]
fn mapping_errors_are_collected() {
  let found = errors(r#"<Button ui:Width="1"><Button.Tag a="1"><Grid.Row/></Button.Tag></Button>"#);

  assert_eq!(
    found,
    [
      ("`ui:Width` names no property".to_owned(), "ui:Width"),
      ("a property element can't have attributes".to_owned(), "a"),
      ("a property element can't contain a property element".to_owned(), "Grid.Row"),
    ]
  );
}

// An unknown entity is an error at the entity, wherever it appears.
#[test]
fn unknown_entities_are_errors() {
  let found = errors(r#"<Text Value="&nbsp;">&copy;</Text>"#);

  assert_eq!(found.len(), 2);
  assert_eq!(found[0].1, "&nbsp;");
  assert_eq!(found[1].1, "&copy;");
}
