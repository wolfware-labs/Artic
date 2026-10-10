// Integration tests for `#[derive(XamlType)]`: derived types reach the registry with the
// descriptors their fields describe, and their generated `Default` and `Object` impls work.

use artic_core::event::Event;
use artic_core::property::{Affects, Precedence, Property, PropertyKey};
use artic_core::tree::{NodeId, TreeKind, UiTree};
use artic_macros::XamlType;
use artic_registry::{AttachedTarget, Registry, TypeInfo, Whitespace, XamlType};
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
  let mut tree = UiTree::new();

  let node = (lookup("Border").new)(&mut tree);

  let border = tree.get_as::<Border>(node).expect("a Border node");
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
  let mut tree = UiTree::new();
  let window = (Window::TYPE_INFO.new)(&mut tree);
  let text = (TextBlock::TYPE_INFO.new)(&mut tree);

  tree.add_child(TreeKind::Logical, window, text);
  tree.get_as::<Window>(window).expect("a Window").font_size.set(20.0);

  assert_eq!(tree.get_as::<TextBlock>(text).expect("a TextBlock").font_size.get(), 20.0);
  assert_eq!(TextBlock::TYPE_INFO.properties[0].name, "FontSize");
  assert!(TextBlock::TYPE_INFO.properties[0].inherits);
}

/// A panel owning two attached properties: `Row`, set on its children, and `Label`, set anywhere.
#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui")]
#[xaml(attached(name = "Row", ty = usize, affects = layout, target = children))]
#[xaml(attached(name = "Label", ty = String, default = "unnamed".to_owned(), target = any))]
struct Grid {
  #[xaml]
  columns: Property<usize>,
}

// Attached properties reach the registry, found by their owner and name, with their targets.
#[test]
fn attached_property_is_found_by_owner_and_name() {
  let registry = Registry::from_link_time().expect("no duplicates");
  let grid = lookup("Grid");

  let row = registry.attached_property(grid, "Row").expect("Grid.Row");
  let label = registry.attached_property(grid, "Label").expect("Grid.Label");

  assert_eq!((row.value_type, row.target), ("usize", AttachedTarget::ChildrenOfOwner));
  assert_eq!((label.value_type, label.target), ("String", AttachedTarget::Any));
  assert!(registry.attached_property(grid, "Columns").is_none());
}

// The generated helpers set and read an attached value on another type's node, and a local value
// overrides a style value until it's cleared.
#[test]
fn attached_helpers_set_and_read_with_precedence() {
  let mut tree = UiTree::new();
  let border = (Border::TYPE_INFO.new)(&mut tree);
  assert_eq!(Grid::row(&tree, border), 0);
  assert_eq!(Grid::label(&tree, border), "unnamed");

  Grid::set_row(&mut tree, border, 3);
  assert_eq!(Grid::row(&tree, border), 3);

  let row = tree.attached_property(border, Grid::ROW);
  row.set_at(Precedence::StyleSetter, 2);
  assert_eq!(Grid::row(&tree, border), 3, "a local value overrides a style value");
  row.clear(Precedence::Local);
  assert_eq!(Grid::row(&tree, border), 2);
}

// An attached key carries the metadata its declaration gives.
#[test]
fn attached_key_carries_its_metadata() {
  assert_eq!(Grid::ROW.key().name(), "Row");
  assert_eq!(Grid::ROW.key().affects(), Affects::LAYOUT);
  assert_eq!(Grid::LABEL.key().affects(), Affects::NONE);
  assert_eq!(Grid::LABEL.default_value(), "unnamed");
}

/// A markup extension with one positional argument and an optional named one.
#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui", markup_extension(positional(key)))]
struct Localize {
  #[xaml]
  key: Property<String>,
  #[xaml]
  fallback: Property<Option<String>>,
}

/// A markup extension without arguments.
#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui", markup_extension)]
struct Nothing;

// A markup extension's schema lists its positional arguments in order, and every property as a
// named argument, with the names and value types of its properties.
#[test]
fn markup_extension_schema_is_registered() {
  let schema = |info: &TypeInfo| {
    let extension = info.markup_extension.expect("a markup extension");
    let arguments = |arguments: &[artic_registry::ArgInfo]| {
      arguments
        .iter()
        .map(|argument| (argument.name, argument.value_type))
        .collect::<Vec<_>>()
    };
    (arguments(extension.positional), arguments(extension.named))
  };

  assert_eq!(
    schema(lookup("Localize")),
    (
      vec![("Key", "String")],
      vec![("Key", "String"), ("Fallback", "Option < String >")]
    )
  );
  assert_eq!(schema(lookup("Nothing")), (vec![], vec![]));
  assert!(lookup("Border").markup_extension.is_none());
}
