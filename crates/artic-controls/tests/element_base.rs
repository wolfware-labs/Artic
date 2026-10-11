// ElementBase: its properties, registered through a control that flattens it, and its layout
// helpers.

use artic_controls::{Classes, ElementBase, Visibility};
use artic_core::Object;
use artic_core::property::{Affects, AnyProperty};
use artic_core::tree::{NodeId, TreeKind, UiTree};
use artic_layout::geometry::{Rect, Size, Thickness};
use artic_layout::{HorizontalAlignment, LayoutCx, LayoutManager, LayoutNodes, Layoutable, VerticalAlignment};
use artic_macros::XamlType;
use artic_registry::{PropertyGroup, Registry};
use pretty_assertions::assert_eq;
use std::cell::Cell;

/// A control with nothing but the base.
#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/test")]
struct Probe {
  #[xaml(flatten)]
  base: ElementBase,
}

/// A second control type, to check inheritance across types.
#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/test")]
struct Other {
  #[xaml(flatten)]
  base: ElementBase,
}

/// The markup names of `ElementBase`'s properties.
const NAMES: [&str; 20] = [
  "Width",
  "Height",
  "MinWidth",
  "MinHeight",
  "MaxWidth",
  "MaxHeight",
  "Margin",
  "HorizontalAlignment",
  "VerticalAlignment",
  "Visibility",
  "IsEnabled",
  "Opacity",
  "RenderTransform",
  "ZIndex",
  "Classes",
  "Name",
  "Cursor",
  "Focusable",
  "IsHitTestVisible",
  "ToolTip",
];

// Every ElementBase property is found on a control that flattens it, under the `base` field.
#[test]
fn element_base_properties_registered() {
  let registry = Registry::from_link_time().expect("no duplicates");
  let info = registry
    .type_by_name("https://artic.rs/test", "Probe")
    .expect("Probe is registered");

  for name in NAMES {
    let found = info.property(name).unwrap_or_else(|| panic!("{name} is a property of Probe"));
    assert_eq!(found.group, Some("base"), "{name}");
  }
  assert_eq!(info.property("Margin").unwrap().info.affects, Affects::LAYOUT);
  assert!(info.property("IsEnabled").unwrap().info.inherits);
  assert_eq!(info.groups[0].properties.len(), NAMES.len());
}

// Defaults: no explicit size, unbounded maximum, enabled, opaque, hit-testable, no classes.
#[test]
fn element_base_defaults() {
  let base = ElementBase::default();

  assert_eq!((base.width.get(), base.max_width.get()), (None, f32::INFINITY));
  assert!(base.is_enabled.get() && base.is_hit_test_visible.get());
  assert_eq!(base.opacity.get(), 1.0);
  assert_eq!(base.classes.get(), Classes::default());
}

// Classes parse from space-separated names.
#[test]
fn classes_parse_from_names() {
  let classes: Classes = "primary  large".parse().unwrap();

  assert!(classes.contains("large") && !classes.contains("small"));
}

/// An element with a base and a fixed content size, which measures its children and counts its
/// own measures.
#[derive(Debug)]
struct Boxed {
  base: ElementBase,
  content: Size,
  measured: Cell<u32>,
}

impl Boxed {
  /// A box whose content is `width` × `height`.
  fn new(width: f32, height: f32) -> Self {
    Self {
      base: ElementBase::default(),
      content: Size::new(width, height),
      measured: Cell::new(0),
    }
  }
}

impl Object for Boxed {
  fn visit_properties(&self, visit: &mut dyn FnMut(&dyn AnyProperty)) {
    self.base.visit_properties(visit);
  }
}

impl Layoutable for Boxed {
  fn measure(&self, cx: &mut LayoutCx<'_>, available: Size) -> Size {
    self.measured.set(self.measured.get() + 1);
    self.base.measure(cx, available, |cx, inner| {
      let children: Vec<NodeId> = cx.children().collect();
      for child in children {
        cx.measure(child, inner);
      }
      self.content
    })
  }

  fn arrange(&self, cx: &mut LayoutCx<'_>, slot: Rect) {
    self.base.arrange(cx, slot, |cx, size| {
      let children: Vec<NodeId> = cx.children().collect();
      for child in children {
        cx.arrange(child, Rect::new(0.0, 0.0, size.width, size.height));
      }
    });
  }
}

/// Lays out `Boxed` nodes.
struct Boxes;

impl LayoutNodes for Boxes {
  fn layoutable<'a>(&self, tree: &'a UiTree, node: NodeId) -> Option<&'a dyn Layoutable> {
    tree.get_as::<Boxed>(node).map(|boxed| boxed as &dyn Layoutable)
  }
}

/// Lays out one box in a 200 × 100 slot and returns its bounds.
fn bounds_of(boxed: Boxed) -> Rect {
  let mut tree = UiTree::new();
  let node = tree.alloc(Box::new(boxed));
  let mut layout = LayoutManager::default();
  layout.run(&mut tree, &Boxes, node, Size::new(200.0, 100.0), 1.0, false);
  layout.rect(node).expect("arranged")
}

// With a 10-unit margin, each alignment places a 40 × 20 content against its edge, in the middle,
// or across the slot inside the margin.
#[test]
fn margin_and_alignment_layout() {
  use HorizontalAlignment as H;
  use VerticalAlignment as V;
  let at = |horizontal, vertical| {
    let boxed = Boxed::new(40.0, 20.0);
    boxed.base.margin.set(Thickness::uniform(10.0));
    boxed.base.horizontal_alignment.set(horizontal);
    boxed.base.vertical_alignment.set(vertical);
    bounds_of(boxed)
  };

  assert_eq!(at(H::Left, V::Top), Rect::new(10.0, 10.0, 40.0, 20.0));
  assert_eq!(at(H::Center, V::Center), Rect::new(80.0, 40.0, 40.0, 20.0));
  assert_eq!(at(H::Right, V::Bottom), Rect::new(150.0, 70.0, 40.0, 20.0));
  assert_eq!(at(H::Stretch, V::Stretch), Rect::new(10.0, 10.0, 180.0, 80.0));
}

// The minimum wins over the content and the maximum caps it, in the desired size.
#[test]
fn min_max_clamp_desired_size() {
  let mut tree = UiTree::new();
  let boxed = Boxed::new(10.0, 50.0);
  boxed.base.min_width.set(60.0);
  boxed.base.max_height.set(30.0);
  let node = tree.alloc(Box::new(boxed));
  let mut layout = LayoutManager::default();

  layout.run(&mut tree, &Boxes, node, Size::new(200.0, 100.0), 1.0, false);

  assert_eq!(layout.desired(node), Some(Size::new(60.0, 30.0)));
}

// A collapsed element has a zero desired size and bounds, its child isn't laid out, and a child
// made dirty afterwards isn't measured.
#[test]
fn collapsed_takes_no_space() {
  let mut tree = UiTree::new();
  let parent = tree.alloc(Box::new(Boxed::new(40.0, 20.0)));
  let child = tree.alloc(Box::new(Boxed::new(10.0, 10.0)));
  tree.add_child(TreeKind::Visual, parent, child);
  let mut layout = LayoutManager::default();
  let available = Size::new(200.0, 100.0);
  layout.run(&mut tree, &Boxes, parent, available, 1.0, false);

  tree
    .get_as::<Boxed>(parent)
    .unwrap()
    .base
    .visibility
    .set(Visibility::Collapsed);
  layout.run(&mut tree, &Boxes, parent, available, 1.0, false);
  assert_eq!(layout.desired(parent), Some(Size::ZERO));
  assert_eq!(layout.rect(parent).map(|bounds| bounds.size()), Some(Size::ZERO));
  assert_eq!(layout.rect(child), None);

  LayoutManager::invalidate_measure(&mut tree, child);
  layout.run(&mut tree, &Boxes, parent, available, 1.0, false);
  assert_eq!(tree.get_as::<Boxed>(child).unwrap().measured.get(), 1);
}

// IsEnabled inherits across control types, since every control flattens ElementBase's one key.
#[test]
fn is_enabled_inherits_across_control_types() {
  let mut tree = UiTree::new();
  let probe = tree.alloc(Box::new(Probe::default()));
  let other = tree.alloc(Box::new(Other::default()));
  tree.add_child(TreeKind::Logical, probe, other);

  tree.get_as::<Probe>(probe).unwrap().base.is_enabled.set(false);

  assert!(!tree.get_as::<Other>(other).unwrap().base.is_enabled.get());
}

// With snapping, the content is arranged in the element's snapped box, so a stretched child is
// exactly as wide as its parent's recorded bounds.
#[test]
fn content_is_arranged_in_the_snapped_bounds() {
  let mut tree = UiTree::new();
  let parent = Boxed::new(40.8, 10.0);
  parent.base.horizontal_alignment.set(HorizontalAlignment::Center);
  let parent = tree.alloc(Box::new(parent));
  let child = tree.alloc(Box::new(Boxed::new(0.0, 0.0)));
  tree.add_child(TreeKind::Visual, parent, child);
  let mut layout = LayoutManager::default();

  layout.run(&mut tree, &Boxes, parent, Size::new(100.0, 10.0), 1.0, true);

  let width = |node| layout.rect(node).expect("arranged").width;
  assert_eq!(width(child), width(parent));
}
