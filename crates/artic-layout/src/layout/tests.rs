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
