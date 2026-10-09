// Integration tests for `#[derive(XamlType)]`: derived types reach the registry with the
// descriptors their fields describe, and their generated `Default` and `Object` impls work.

use artic_core::event::Event;
use artic_core::property::{Affects, Precedence, Property, PropertyKey};
use artic_core::tree::{NodeId, Tree, TreeArena};
use artic_macros::XamlType;
use artic_registry::{Registry, TypeInfo, Whitespace, XamlType};
use pretty_assertions::assert_eq;
use std::str::FromStr;

const UI: &str = "https://artic.rs/ui";

// Placeholders for value types that later issues add: geometry (#22), brushes (#35), pointer input
// and the element base (#42). They only need to parse from text and have defaults.

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Thickness(f32);

impl Thickness {
  const ZERO: Thickness = Thickness(0.0);
}

impl FromStr for Thickness {
  type Err = std::num::ParseFloatError;

  fn from_str(text: &str) -> Result<Self, Self::Err> {
    text.parse().map(Thickness)
  }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct CornerRadius(f32);

impl FromStr for CornerRadius {
  type Err = std::num::ParseFloatError;

  fn from_str(text: &str) -> Result<Self, Self::Err> {
    text.parse().map(CornerRadius)
  }
}

#[derive(Debug, Clone, PartialEq)]
struct Brush(String);

impl FromStr for Brush {
  type Err = std::convert::Infallible;

  fn from_str(text: &str) -> Result<Self, Self::Err> {
    Ok(Brush(text.to_owned()))
  }
}

#[derive(Debug)]
struct PointerEvent;

#[derive(Debug, Default)]
struct ElementBase;

/// The `Border` from the spec's §3, with `affects` added.
#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui", content = child)]
struct Border {
  _base: ElementBase,
  #[xaml]
  child: Property<Option<NodeId>>,
  #[xaml(default = Thickness::ZERO, affects = layout)]
  padding: Property<Thickness>,
  #[xaml(affects = render)]
  background: Property<Option<Brush>>,
  #[xaml(affects = render)]
  corner_radius: Property<CornerRadius>,
  #[xaml]
  pointer_pressed: Event<PointerEvent>,
}

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui", content = children)]
struct StackPanel {
  #[xaml]
  children: Vec<NodeId>,
}

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui", content = text, whitespace = "significant")]
struct Run {
  #[xaml]
  text: Property<String>,
}

static FONT_SIZE: PropertyKey<f32> = PropertyKey::new("FontSize", Affects::LAYOUT, true);

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui")]
struct Window {
  #[xaml(key = FONT_SIZE, default = 14.0)]
  font_size: Property<f32>,
}

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui")]
struct TextBlock {
  #[xaml(key = FONT_SIZE)]
  font_size: Property<f32>,
}

/// Looks `name` up in the default namespace of a registry built from every linked type.
fn lookup(name: &str) -> &'static TypeInfo {
  let registry = Registry::from_link_time().expect("no duplicates");
  registry.type_by_name(UI, name).expect("the type is registered")
}

// The spec's `Border` is registered with its four properties, its event and its content field.
#[test]
fn border_is_registered_with_its_members() {
  let border = lookup("Border");

  let properties: Vec<(&str, &str, Affects)> = border
    .properties
    .iter()
    .map(|property| (property.name, property.field, property.affects))
    .collect();
  assert_eq!(
    properties,
    [
      ("Child", "child", Affects::NONE),
      ("Padding", "padding", Affects::LAYOUT),
      ("Background", "background", Affects::RENDER),
      ("CornerRadius", "corner_radius", Affects::RENDER),
    ]
  );
  assert_eq!(
    border.events.iter().map(|event| event.name).collect::<Vec<_>>(),
    ["PointerPressed"]
  );
  assert_eq!(border.content, Some("child"));
  assert!(!border.is_collection);
  assert!(border.rust_path.ends_with("::Border"));
  assert!(std::ptr::eq(border, Border::TYPE_INFO));
}

// The descriptor's constructor creates a node of the type with its default values.
#[test]
fn new_creates_a_node_with_defaults() {
  let mut arena = TreeArena::new();

  let node = (lookup("Border").new)(&mut arena);

  let border = arena.get_as::<Border>(node).expect("a Border node");
  assert_eq!(border.child.get(), None);
  assert_eq!(border.background.get(), None);
  assert_eq!(border.corner_radius.get(), CornerRadius(0.0));
  let _ = &border.pointer_pressed;
}

// `default = expr` fills the property's Default level.
#[test]
fn default_expression_sets_the_default_level() {
  let border = Border::default();

  assert_eq!(border.padding.get(), Thickness::ZERO);
  assert_eq!(border.padding.effective_source(), Precedence::Default);
  assert_eq!(Window::default().font_size.get(), 14.0, "differs from f32's own default");
}

// A type with a collection field accepts children.
#[test]
fn collection_field_makes_the_type_a_collection() {
  let panel = lookup("StackPanel");

  assert!(panel.is_collection);
  assert_eq!(panel.content, Some("children"));
  assert!(StackPanel::default().children.is_empty());
}

// `whitespace = "significant"` reaches the descriptor.
#[test]
fn whitespace_setting_reaches_the_descriptor() {
  assert_eq!(lookup("Run").whitespace, Whitespace::Significant);
  assert_eq!(lookup("Border").whitespace, Whitespace::Collapse);
}

// Two types that share a key inherit from each other through the logical tree, and the key's
// metadata reaches both descriptors.
#[test]
fn shared_key_inherits_across_types() {
  let mut arena = TreeArena::new();
  let window = (Window::TYPE_INFO.new)(&mut arena);
  let text = (TextBlock::TYPE_INFO.new)(&mut arena);

  arena.add_child(Tree::Logical, window, text);
  arena.get_as::<Window>(window).expect("a Window").font_size.set(20.0);

  assert_eq!(arena.get_as::<TextBlock>(text).expect("a TextBlock").font_size.get(), 20.0);
  assert_eq!(TextBlock::TYPE_INFO.properties[0].name, "FontSize");
  assert!(TextBlock::TYPE_INFO.properties[0].inherits);
}
