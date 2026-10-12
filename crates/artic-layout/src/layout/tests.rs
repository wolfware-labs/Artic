use super::*;
use artic_core::Object;
use artic_core::property::AnyProperty;
use pretty_assertions::assert_eq;
use std::cell::Cell;

/// A test node: a leaf of a fixed size, or a stack of its children from top to bottom. It counts
/// how often it's measured and arranged.
#[derive(Debug)]
struct Node {
  stack: bool,
  size: Cell<Size>,
  measured: Cell<u32>,
  arranged: Cell<u32>,
}

impl Object for Node {
  fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
}

impl Layoutable for Node {
  fn measure(&self, cx: &mut LayoutCx<'_>, available: Size) -> Size {
    self.measured.set(self.measured.get() + 1);
    if !self.stack {
      return self.size.get();
    }
    let mut total = Size::ZERO;
    for child in cx.children() {
      let desired = cx.measure(child, Size::new(available.width, f32::INFINITY));
      total = Size::new(total.width.max(desired.width), total.height + desired.height);
    }
    total
  }

  fn arrange(&self, cx: &mut LayoutCx<'_>, rect: Rect) {
    self.arranged.set(self.arranged.get() + 1);
    let mut top = 0.0;
    for child in cx.children() {
      let height = cx.desired(child).height;
      cx.arrange(child, Rect::new(0.0, top, rect.width, height));
      top += height;
    }
  }
}

struct Nodes;

impl LayoutNodes for Nodes {
  fn layoutable<'a>(&self, tree: &'a UiTree, node: NodeId) -> Option<&'a dyn Layoutable> {
    tree.get_as::<Node>(node).map(|node| node as &dyn Layoutable)
  }
}

fn leaf(tree: &mut UiTree, width: f32, height: f32) -> NodeId {
  tree.alloc(Box::new(Node {
    stack: false,
    size: Cell::new(Size::new(width, height)),
    measured: Cell::new(0),
    arranged: Cell::new(0),
  }))
}

fn stack(tree: &mut UiTree, children: &[NodeId]) -> NodeId {
  let node = tree.alloc(Box::new(Node {
    stack: true,
    size: Cell::new(Size::ZERO),
    measured: Cell::new(0),
    arranged: Cell::new(0),
  }));
  for child in children {
    tree.add_child(TreeKind::Visual, node, *child);
  }
  node
}

fn node(tree: &UiTree, id: NodeId) -> &Node {
  tree.get_as::<Node>(id).expect("a test node")
}

/// How often each of `ids` was measured and arranged.
fn counts(tree: &UiTree, ids: &[NodeId]) -> Vec<(u32, u32)> {
  ids
    .iter()
    .map(|id| (node(tree, *id).measured.get(), node(tree, *id).arranged.get()))
    .collect()
}

fn run(manager: &mut LayoutManager, tree: &mut UiTree, root: NodeId, available: Size) {
  manager.run(tree, &Nodes, root, available, 1.0, false);
}

const WINDOW: Size = Size::new(400.0, 300.0);

// Measuring a node again in the same available size, with nothing changed, doesn't call it.
#[test]
fn measure_cached_for_same_available() {
  let mut tree = UiTree::new();
  let child = leaf(&mut tree, 100.0, 20.0);
  let root = stack(&mut tree, &[child]);
  let mut manager = LayoutManager::default();

  run(&mut manager, &mut tree, root, WINDOW);
  run(&mut manager, &mut tree, root, WINDOW);

  assert_eq!(counts(&tree, &[root, child]), [(1, 1), (1, 1)]);
  assert_eq!(manager.desired(root), Some(Size::new(100.0, 20.0)));
}

// A different available size measures again, and the new results are kept.
#[test]
fn measure_reruns_on_available_change() {
  let mut tree = UiTree::new();
  let child = leaf(&mut tree, 100.0, 20.0);
  let root = stack(&mut tree, &[child]);
  let mut manager = LayoutManager::default();

  run(&mut manager, &mut tree, root, WINDOW);
  run(&mut manager, &mut tree, root, Size::new(500.0, 300.0));

  assert_eq!(counts(&tree, &[root, child]), [(2, 2), (2, 2)]);
  assert_eq!(manager.rect(child), Some(Rect::new(0.0, 0.0, 500.0, 20.0)));
}

// A node marked for layout whose size doesn't change is measured and arranged again on its own:
// its parent and siblings keep their results.
#[test]
fn unchanged_desired_size_stops_propagation() {
  let mut tree = UiTree::new();
  let [first, second] = [leaf(&mut tree, 100.0, 20.0), leaf(&mut tree, 100.0, 30.0)];
  let root = stack(&mut tree, &[first, second]);
  let mut manager = LayoutManager::default();
  run(&mut manager, &mut tree, root, WINDOW);

  LayoutManager::invalidate_measure(&mut tree, first);
  run(&mut manager, &mut tree, root, WINDOW);

  assert_eq!(counts(&tree, &[root, first, second]), [(1, 1), (2, 2), (1, 1)]);
}

// A node whose size changes measures its parent again, which moves the siblings after it.
#[test]
fn changed_desired_size_propagates_to_the_parent() {
  let mut tree = UiTree::new();
  let [first, second] = [leaf(&mut tree, 100.0, 20.0), leaf(&mut tree, 100.0, 30.0)];
  let root = stack(&mut tree, &[first, second]);
  let mut manager = LayoutManager::default();
  run(&mut manager, &mut tree, root, WINDOW);

  node(&tree, first).size.set(Size::new(100.0, 50.0));
  LayoutManager::invalidate_measure(&mut tree, first);
  run(&mut manager, &mut tree, root, WINDOW);

  assert_eq!(counts(&tree, &[root, first, second]), [(2, 2), (2, 2), (1, 2)]);
  assert_eq!(manager.rect(second), Some(Rect::new(0.0, 50.0, 400.0, 30.0)));
}

// Removing a visual child lays its parent out again, and drops the child's results.
#[test]
fn removed_children_relayout_the_parent() {
  let mut tree = UiTree::new();
  let [first, second] = [leaf(&mut tree, 100.0, 20.0), leaf(&mut tree, 100.0, 30.0)];
  let root = stack(&mut tree, &[first, second]);
  let mut manager = LayoutManager::default();
  run(&mut manager, &mut tree, root, WINDOW);

  tree.detach(TreeKind::Visual, first);
  run(&mut manager, &mut tree, root, WINDOW);

  assert_eq!(manager.rect(second), Some(Rect::new(0.0, 0.0, 400.0, 30.0)));
  assert_eq!(manager.desired(first), None);
}

// Arrange-only invalidation arranges the node again in the same rectangle, without measuring.
#[test]
fn invalidate_arrange_skips_measure() {
  let mut tree = UiTree::new();
  let child = leaf(&mut tree, 100.0, 20.0);
  let root = stack(&mut tree, &[child]);
  let mut manager = LayoutManager::default();
  run(&mut manager, &mut tree, root, WINDOW);

  manager.invalidate_arrange(child);
  run(&mut manager, &mut tree, root, WINDOW);

  assert_eq!(counts(&tree, &[root, child]), [(1, 1), (1, 2)]);
}

// An infinite available size is legal: the stack offers its children infinite height.
#[test]
fn infinite_available_is_legal() {
  let mut tree = UiTree::new();
  let child = leaf(&mut tree, 100.0, 20.0);
  let root = stack(&mut tree, &[child]);
  let mut manager = LayoutManager::default();

  run(&mut manager, &mut tree, root, Size::INFINITY);

  assert_eq!(manager.desired(child), Some(Size::new(100.0, 20.0)));
  assert_eq!(
    manager.rect(root),
    Some(Rect::new(0.0, 0.0, 100.0, 20.0)),
    "an infinite root takes its desired size"
  );
}

// An element that asks for an infinite size is a bug, reported with the node in debug builds.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "returned an invalid desired size")]
fn infinite_desired_size_panics_in_debug() {
  let mut tree = UiTree::new();
  let child = leaf(&mut tree, f32::INFINITY, 20.0);
  let root = stack(&mut tree, &[child]);

  run(&mut LayoutManager::default(), &mut tree, root, WINDOW);
}

// The panic names the node that returned the infinite size.
#[test]
#[cfg(debug_assertions)]
fn infinite_desired_size_names_the_node() {
  let mut tree = UiTree::new();
  let child = leaf(&mut tree, f32::INFINITY, 20.0);
  let root = stack(&mut tree, &[child]);

  let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    run(&mut LayoutManager::default(), &mut tree, root, WINDOW);
  }))
  .expect_err("panics");

  let message = panic.downcast_ref::<String>().expect("a formatted message");
  assert!(message.starts_with(&format!("node {child:?} returned")), "{message}");
}

// With snapping on, arranged rectangles have their edges on whole device pixels.
#[test]
fn layout_rounding_snaps_to_device_pixels() {
  for scale in [1.0_f32, 1.25, 2.0] {
    let mut tree = UiTree::new();
    let children = [10.3, 10.3, 10.3].map(|height| leaf(&mut tree, 33.3, height));
    let root = stack(&mut tree, &children);
    let mut manager = LayoutManager::default();

    manager.run(&mut tree, &Nodes, root, Size::new(100.3, 200.0), scale, true);

    for child in children {
      let rect = manager.rect(child).expect("arranged");
      for edge in [rect.x, rect.y, rect.x + rect.width, rect.y + rect.height] {
        let device = edge * scale;
        assert!((device - device.round()).abs() < 1e-3, "{rect} at scale {scale}");
      }
    }
  }
}

mod bounds {
  use crate::geometry::{Rect, Size};
  use crate::{LayoutCx, LayoutManager, LayoutNodes, Layoutable};
  use artic_core::Object;
  use artic_core::property::AnyProperty;
  use artic_core::tree::{NodeId, TreeKind, UiTree};
  use pretty_assertions::assert_eq;
  use std::cell::Cell;

  /// Takes a 10-unit inset of its slot as its bounds, and fills them with its children.
  #[derive(Debug, Default)]
  struct Inset {
    arranged: Cell<u32>,
  }

  impl Object for Inset {
    fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  }

  impl Layoutable for Inset {
    fn measure(&self, cx: &mut LayoutCx<'_>, available: Size) -> Size {
      let children: Vec<NodeId> = cx.children().collect();
      for child in children {
        cx.measure(child, available);
      }
      available
    }

    fn arrange(&self, cx: &mut LayoutCx<'_>, slot: Rect) {
      self.arranged.set(self.arranged.get() + 1);
      let bounds = Rect::new(slot.x + 10.0, slot.y + 10.0, slot.width - 20.0, slot.height - 20.0);
      cx.set_bounds(bounds);
      let children: Vec<NodeId> = cx.children().collect();
      for child in children {
        cx.arrange(child, Rect::new(0.0, 0.0, bounds.width, bounds.height));
      }
    }
  }

  /// Lays out `Inset` nodes.
  struct Insets;

  impl LayoutNodes for Insets {
    fn layoutable<'a>(&self, tree: &'a UiTree, node: NodeId) -> Option<&'a dyn Layoutable> {
      tree.get_as::<Inset>(node).map(|inset| inset as &dyn Layoutable)
    }
  }

  // An element's bounds are what it set, its children are placed relative to them, and the same
  // slot doesn't arrange it again although its bounds differ from the slot.
  #[test]
  fn set_bounds_records_the_element_box() {
    let mut tree = UiTree::new();
    let outer = tree.alloc(Box::<Inset>::default());
    let inner = tree.alloc(Box::<Inset>::default());
    tree.add_child(TreeKind::Visual, outer, inner);
    let mut layout = LayoutManager::default();

    layout.run(&mut tree, &Insets, outer, Size::new(100.0, 80.0), 1.0, false);
    assert_eq!(layout.rect(outer), Some(Rect::new(10.0, 10.0, 80.0, 60.0)));
    assert_eq!(layout.rect(inner), Some(Rect::new(10.0, 10.0, 60.0, 40.0)));

    LayoutManager::invalidate_measure(&mut tree, outer);
    layout.run(&mut tree, &Insets, outer, Size::new(100.0, 80.0), 1.0, false);
    let arranged = |node| tree.get_as::<Inset>(node).unwrap().arranged.get();
    assert_eq!(arranged(inner), 1, "same slot, nothing changed below: not arranged again");
  }
}

mod skipped {
  use crate::geometry::{Rect, Size};
  use crate::{LayoutCx, LayoutManager, LayoutNodes, Layoutable};
  use artic_core::Object;
  use artic_core::property::AnyProperty;
  use artic_core::tree::{NodeId, TreeKind, UiTree};
  use pretty_assertions::assert_eq;
  use std::cell::Cell;

  /// Measures its children unless `skip` is set, as a collapsed element does; counts its own
  /// measures.
  #[derive(Debug, Default)]
  struct Toggle {
    skip: Cell<bool>,
    measured: Cell<u32>,
  }

  impl Object for Toggle {
    fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  }

  impl Layoutable for Toggle {
    fn measure(&self, cx: &mut LayoutCx<'_>, available: Size) -> Size {
      self.measured.set(self.measured.get() + 1);
      if self.skip.get() {
        return Size::ZERO;
      }
      let children: Vec<NodeId> = cx.children().collect();
      for child in children {
        cx.measure(child, available);
      }
      Size::new(10.0, 10.0)
    }

    fn arrange(&self, cx: &mut LayoutCx<'_>, rect: Rect) {
      if self.skip.get() {
        return;
      }
      let children: Vec<NodeId> = cx.children().collect();
      for child in children {
        cx.arrange(child, Rect::new(0.0, 0.0, rect.width, rect.height));
      }
    }
  }

  /// Lays out `Toggle` nodes.
  struct Toggles;

  impl LayoutNodes for Toggles {
    fn layoutable<'a>(&self, tree: &'a UiTree, node: NodeId) -> Option<&'a dyn Layoutable> {
      tree.get_as::<Toggle>(node).map(|toggle| toggle as &dyn Layoutable)
    }
  }

  // Once a parent stops measuring a child, the child and its subtree are forgotten, and a child
  // made dirty afterwards isn't laid out.
  #[test]
  fn children_a_measure_skips_are_forgotten() {
    let mut tree = UiTree::new();
    let [parent, child, grandchild] = [(); 3].map(|()| tree.alloc(Box::<Toggle>::default()));
    tree.add_child(TreeKind::Visual, parent, child);
    tree.add_child(TreeKind::Visual, child, grandchild);
    let mut layout = LayoutManager::default();
    let available = Size::new(100.0, 100.0);
    layout.run(&mut tree, &Toggles, parent, available, 1.0, false);
    assert!(layout.rect(grandchild).is_some());

    tree.get_as::<Toggle>(parent).unwrap().skip.set(true);
    LayoutManager::invalidate_measure(&mut tree, parent);
    layout.run(&mut tree, &Toggles, parent, available, 1.0, false);
    assert_eq!((layout.rect(child), layout.rect(grandchild)), (None, None));

    LayoutManager::invalidate_measure(&mut tree, child);
    layout.run(&mut tree, &Toggles, parent, available, 1.0, false);
    assert_eq!(tree.get_as::<Toggle>(child).unwrap().measured.get(), 1);
  }

  // A child whose parent stopped measuring it, as a collapsed parent does, keeps its last size,
  // so when the parent shows it again at another size the child is listed as resized.
  #[test]
  fn child_shown_again_at_another_size_is_resized() {
    let mut tree = UiTree::new();
    let [parent, child] = [(); 2].map(|()| tree.alloc(Box::<Toggle>::default()));
    tree.add_child(TreeKind::Visual, parent, child);
    let mut layout = LayoutManager::default();
    layout.run(&mut tree, &Toggles, parent, Size::new(100.0, 100.0), 1.0, false);
    tree.get_as::<Toggle>(parent).unwrap().skip.set(true);
    LayoutManager::invalidate_measure(&mut tree, parent);
    layout.run(&mut tree, &Toggles, parent, Size::new(200.0, 200.0), 1.0, false);

    tree.get_as::<Toggle>(parent).unwrap().skip.set(false);
    LayoutManager::invalidate_measure(&mut tree, parent);
    let stats = layout.run(&mut tree, &Toggles, parent, Size::new(200.0, 200.0), 1.0, false);

    assert_eq!(stats.resized, [child]);
  }

  /// A root, a middle node and a leaf, laid out once.
  fn three_levels() -> (UiTree, LayoutManager, [NodeId; 3]) {
    let mut tree = UiTree::new();
    let nodes = [(); 3].map(|()| tree.alloc(Box::<Toggle>::default()));
    tree.add_child(TreeKind::Visual, nodes[0], nodes[1]);
    tree.add_child(TreeKind::Visual, nodes[1], nodes[2]);
    let mut layout = LayoutManager::default();
    layout.run(&mut tree, &Toggles, nodes[0], Size::new(100.0, 100.0), 1.0, false);
    (tree, layout, nodes)
  }

  /// How many times `node` has measured.
  fn measures(tree: &UiTree, node: NodeId) -> u32 {
    tree.get_as::<Toggle>(node).unwrap().measured.get()
  }

  // A dirty node under a dirty ancestor waits for the ancestor's measure, so when the ancestor
  // stops measuring its children in the same run, the node isn't measured at all.
  #[test]
  fn dirty_descendant_of_a_collapsing_ancestor_isnt_measured() {
    let (mut tree, mut layout, [root, middle, leaf]) = three_levels();

    tree.get_as::<Toggle>(middle).unwrap().skip.set(true);
    LayoutManager::invalidate_measure(&mut tree, middle);
    LayoutManager::invalidate_measure(&mut tree, leaf);
    layout.run(&mut tree, &Toggles, root, Size::new(100.0, 100.0), 1.0, false);

    assert_eq!(measures(&tree, leaf), 1);
    assert_eq!(layout.rect(leaf), None);
  }

  // When the ancestor keeps measuring, its measure still reaches the dirty node through a clean
  // node in between, which would otherwise answer from its cache.
  #[test]
  fn dirty_descendant_of_a_dirty_ancestor_is_still_measured() {
    let (mut tree, mut layout, [root, _, leaf]) = three_levels();

    LayoutManager::invalidate_measure(&mut tree, root);
    LayoutManager::invalidate_measure(&mut tree, leaf);
    layout.run(&mut tree, &Toggles, root, Size::new(100.0, 100.0), 1.0, false);

    assert_eq!(measures(&tree, leaf), 2);
  }
}

mod misuse {
  use crate::geometry::{Rect, Size};
  use crate::{LayoutCx, LayoutManager, LayoutNodes, Layoutable};
  use artic_core::Object;
  use artic_core::property::AnyProperty;
  use artic_core::tree::{NodeId, TreeKind, UiTree};

  /// Measures its first grandchild directly, skipping the child in between.
  #[derive(Debug)]
  struct Skipper;

  impl Object for Skipper {
    fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  }

  impl Layoutable for Skipper {
    fn measure(&self, cx: &mut LayoutCx<'_>, available: Size) -> Size {
      let tree = cx.tree();
      let grandchild = cx.children().find_map(|child| tree.children(TreeKind::Visual, child).next());
      if let Some(grandchild) = grandchild {
        cx.measure(grandchild, available);
      }
      Size::ZERO
    }

    fn arrange(&self, _cx: &mut LayoutCx<'_>, _rect: Rect) {}
  }

  /// Lays out `Skipper` nodes.
  struct Skippers;

  impl LayoutNodes for Skippers {
    fn layoutable<'a>(&self, tree: &'a UiTree, node: NodeId) -> Option<&'a dyn Layoutable> {
      tree.get_as::<Skipper>(node).map(|skipper| skipper as &dyn Layoutable)
    }
  }

  /// Calls `set_bounds` while measuring, which only `arrange` may.
  #[derive(Debug)]
  struct EarlyBounds;

  impl Object for EarlyBounds {
    fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  }

  impl Layoutable for EarlyBounds {
    fn measure(&self, cx: &mut LayoutCx<'_>, _available: Size) -> Size {
      cx.set_bounds(Rect::new(0.0, 0.0, 1.0, 1.0));
      Size::ZERO
    }

    fn arrange(&self, _cx: &mut LayoutCx<'_>, _rect: Rect) {}
  }

  /// Lays out `EarlyBounds` nodes.
  struct EarlyBoundsNodes;

  impl LayoutNodes for EarlyBoundsNodes {
    fn layoutable<'a>(&self, tree: &'a UiTree, node: NodeId) -> Option<&'a dyn Layoutable> {
      tree.get_as::<EarlyBounds>(node).map(|node| node as &dyn Layoutable)
    }
  }

  // set_bounds belongs to arrange: calling it while measuring panics.
  #[test]
  #[should_panic(expected = "called from a node's arrange")]
  fn set_bounds_while_measuring_panics() {
    let mut tree = UiTree::new();
    let node = tree.alloc(Box::new(EarlyBounds));

    LayoutManager::default().run(&mut tree, &EarlyBoundsNodes, node, Size::new(10.0, 10.0), 1.0, false);
  }

  // Measuring a node that isn't one of the current node's visual children is a bug, caught in
  // debug builds: its results would be forgotten right after.
  #[test]
  #[cfg(debug_assertions)]
  #[should_panic(expected = "isn't a visual child of")]
  fn measuring_a_grandchild_panics() {
    let mut tree = UiTree::new();
    let [root, child, grandchild] = [(); 3].map(|()| tree.alloc(Box::new(Skipper)));
    tree.add_child(TreeKind::Visual, root, child);
    tree.add_child(TreeKind::Visual, child, grandchild);

    LayoutManager::default().run(&mut tree, &Skippers, root, Size::new(10.0, 10.0), 1.0, false);
  }
}

// A run reports the nodes it measured and arranged, lists none as resized the first time, and a
// run with nothing changed reports nothing.
#[test]
fn run_reports_measured_and_arranged() {
  let mut tree = UiTree::new();
  let child = leaf(&mut tree, 100.0, 20.0);
  let root = stack(&mut tree, &[child]);
  let mut manager = LayoutManager::default();

  let first = manager.run(&mut tree, &Nodes, root, WINDOW, 1.0, false);
  let second = manager.run(&mut tree, &Nodes, root, WINDOW, 1.0, false);

  assert_eq!((first.measured, first.arranged), (2, 2));
  assert!(first.resized.is_empty());
  assert_eq!(second, LayoutStats::default());
}

// A node whose arranged size changes is listed as resized; a sibling that only moves isn't.
#[test]
fn run_reports_resized() {
  let mut tree = UiTree::new();
  let grows = leaf(&mut tree, 100.0, 20.0);
  let moves = leaf(&mut tree, 100.0, 20.0);
  let root = stack(&mut tree, &[grows, moves]);
  let mut manager = LayoutManager::default();
  run(&mut manager, &mut tree, root, WINDOW);

  node(&tree, grows).size.set(Size::new(100.0, 40.0));
  LayoutManager::invalidate_measure(&mut tree, grows);
  let stats = manager.run(&mut tree, &Nodes, root, WINDOW, 1.0, false);

  assert_eq!(stats.resized, [grows]);
}
