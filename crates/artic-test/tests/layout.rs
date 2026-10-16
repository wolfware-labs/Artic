// The headless layout helpers, with the shared test elements and elements that break the rules
// on purpose.

use artic_core::tree::{NodeId, TreeKind};
use artic_layout::LayoutManager;
use artic_layout::geometry::{Rect, Size};
use artic_test::layout::{Boxes, Fixed, Headless, Stack};
use pretty_assertions::assert_eq;

const WINDOW: Size = Size::new(400.0, 300.0);

// A stack places its children top to bottom at its own width and asks for the widest child and
// the sum of the heights.
#[test]
fn headless_lays_out_a_stack() {
  let mut layout = Headless::new(Boxes);
  let first = layout.add("first", Fixed::new(100.0, 20.0));
  let second = layout.add("second", Fixed::new(80.0, 30.0));
  let root = layout.parent("stack", Stack::new(), &[first, second]);

  layout.run(root, WINDOW, 1.0);

  assert_eq!(layout.manager().desired(root), Some(Size::new(100.0, 50.0)));
  assert_eq!(layout.manager().rect(first), Some(Rect::new(0.0, 0.0, 400.0, 20.0)));
  assert_eq!(layout.manager().rect(second), Some(Rect::new(0.0, 20.0, 400.0, 30.0)));
}

// `parent` links the children as visual children, in the order given.
#[test]
fn parent_links_children_in_order() {
  let mut layout = Headless::new(Boxes);
  let first = layout.add("first", Fixed::new(1.0, 1.0));
  let second = layout.add("second", Fixed::new(1.0, 1.0));
  let root = layout.parent("stack", Stack::new(), &[second, first]);

  let children: Vec<_> = layout.tree().children(TreeKind::Visual, root).collect();

  assert_eq!(children, [second, first]);
}

// The elements count each measure and arrange, so tests can check what layout skipped.
#[test]
fn elements_count_their_calls() {
  let mut layout = Headless::new(Boxes);
  let leaf = layout.add("leaf", Fixed::new(10.0, 10.0));
  let root = layout.parent("stack", Stack::new(), &[leaf]);

  layout.run(root, WINDOW, 1.0);
  layout.run(root, WINDOW, 1.0);

  let tree = layout.tree();
  let fixed = tree.get_as::<Fixed>(leaf).expect("a fixed");
  let stack = tree.get_as::<Stack>(root).expect("a stack");
  assert_eq!((fixed.measured(), fixed.arranged()), (1, 1));
  assert_eq!((stack.measured(), stack.arranged()), (1, 1));
}

// A new size on a `Fixed` shows after its node is invalidated.
#[test]
fn set_size_shows_after_invalidating() {
  let mut layout = Headless::new(Boxes);
  let leaf = layout.add("leaf", Fixed::new(10.0, 10.0));
  let root = layout.parent("stack", Stack::new(), &[leaf]);
  layout.run(root, WINDOW, 1.0);

  layout.tree().get_as::<Fixed>(leaf).expect("a fixed").set_size(Size::new(10.0, 40.0));
  LayoutManager::invalidate_measure(layout.tree(), leaf);
  layout.run(root, WINDOW, 1.0);

  assert_eq!(layout.manager().desired(root), Some(Size::new(10.0, 40.0)));
}

// `run` snaps to the device grid: at 1.25, a 10.3 tall child ends at 13 device pixels, 10.4.
#[test]
fn run_snaps_to_the_device_grid() {
  let mut layout = Headless::new(Boxes);
  let first = layout.add("first", Fixed::new(100.0, 10.3));
  let second = layout.add("second", Fixed::new(100.0, 10.0));
  let root = layout.parent("stack", Stack::new(), &[first, second]);

  layout.run(root, WINDOW, 1.25);

  let rect = layout.manager().rect(second).expect("arranged");
  assert!((rect.y - 10.4).abs() < 0.001, "{rect}");
}

// With snapping off, rectangles keep their fractions.
#[test]
fn set_snap_false_keeps_fractions() {
  let mut layout = Headless::new(Boxes);
  let first = layout.add("first", Fixed::new(100.0, 10.3));
  let second = layout.add("second", Fixed::new(100.0, 10.0));
  let root = layout.parent("stack", Stack::new(), &[first, second]);

  layout.set_snap(false);
  layout.run(root, WINDOW, 1.25);

  assert_eq!(layout.manager().rect(second), Some(Rect::new(0.0, 10.3, 400.0, 10.0)));
}

// A node made through the tree directly has no label and shows as its id.
#[test]
fn nodes_without_a_label_show_their_id() {
  let mut layout = Headless::new(Boxes);
  let labeled = layout.add("leaf", Fixed::new(1.0, 1.0));
  let bare = layout.tree().alloc(Box::new(Fixed::new(1.0, 1.0)));

  assert_eq!(layout.label(labeled), "leaf");
  assert_eq!(layout.label(bare), format!("{bare:?}"));
}

fn two_in_a_stack() -> (Headless<Boxes>, [NodeId; 3]) {
  let mut layout = Headless::new(Boxes);
  let first = layout.add("first", Fixed::new(100.0, 20.0));
  let second = layout.add("second", Fixed::new(80.0, 24.0));
  let root = layout.parent("stack", Stack::new(), &[first, second]);
  layout.run(root, WINDOW, 1.0);
  (layout, [root, first, second])
}

// Matching sizes and rectangles pass.
#[test]
fn matching_assertions_pass() {
  let (layout, [root, _, second]) = two_in_a_stack();

  layout.assert_desired(root, Size::new(100.0, 44.0));
  layout.assert_rect(second, Rect::new(0.0, 20.0, 400.0, 24.0));
}

// A wrong rectangle names the node and shows both values.
#[test]
#[should_panic(expected = r#""second": rect expected 0,20,400,30, got 0,20,400,24"#)]
fn failing_assertion_names_the_node() {
  let (layout, [_, _, second]) = two_in_a_stack();

  layout.assert_rect(second, Rect::new(0.0, 20.0, 400.0, 30.0));
}

// A wrong desired size reads the same way.
#[test]
#[should_panic(expected = r#""first": desired expected 100,30, got 100,20"#)]
fn failing_desired_names_the_node() {
  let (layout, [_, first, _]) = two_in_a_stack();

  layout.assert_desired(first, Size::new(100.0, 30.0));
}

// A node outside the laid-out subtree was never measured, and says so.
#[test]
#[should_panic(expected = r#""orphan": not measured"#)]
fn unplaced_node_is_not_measured() {
  let (mut layout, _) = two_in_a_stack();
  let orphan = layout.add("orphan", Fixed::new(1.0, 1.0));

  layout.assert_desired(orphan, Size::new(1.0, 1.0));
}

// The same node was never arranged either.
#[test]
#[should_panic(expected = r#""orphan": not arranged"#)]
fn unplaced_node_is_not_arranged() {
  let (mut layout, _) = two_in_a_stack();
  let orphan = layout.add("orphan", Fixed::new(1.0, 1.0));

  layout.assert_rect(orphan, Rect::new(0.0, 0.0, 1.0, 1.0));
}

// The default tolerance absorbs f32 error; a tighter one given to `_within` doesn't.
#[test]
#[should_panic(expected = r#""first": rect expected 0,0,400,20.0005"#)]
fn tighter_tolerance_fails() {
  let (layout, [_, first, _]) = two_in_a_stack();
  layout.assert_rect(first, Rect::new(0.0, 0.0, 400.0, 20.0005));

  layout.assert_rect_within(first, Rect::new(0.0, 0.0, 400.0, 20.0005), 0.0001);
}

// Expecting infinity from a finite result fails.
#[test]
#[should_panic(expected = r#""first": desired expected inf,20, got 100,20"#)]
fn infinite_expectation_fails() {
  let (layout, [_, first, _]) = two_in_a_stack();

  layout.assert_desired_within(first, Size::new(f32::INFINITY, 20.0), f32::MAX);
}
