use crate::Object;
use crate::property::{Affects, AnyProperty, AttachedKey, Precedence};
use crate::tree::{NodeId, UiTree};
use pretty_assertions::assert_eq;

static ROW: AttachedKey<usize> = AttachedKey::new("Row", Affects::LAYOUT, || 0);
static LABEL: AttachedKey<String> = AttachedKey::new("Label", Affects::RENDER, || "none".to_owned());

#[derive(Debug)]
struct Button;

impl Object for Button {
  fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
}

/// A tree with one button whose dirty flags are already cleared.
fn clean_button() -> (UiTree, NodeId) {
  let mut tree = UiTree::new();
  let button = tree.alloc(Box::new(Button));
  tree.clear_dirty(button, Affects::LAYOUT | Affects::RENDER);
  (tree, button)
}

// Reading an attached property nothing set returns its default and stores nothing.
#[test]
fn unset_value_is_the_default_without_storing() {
  let (tree, button) = clean_button();

  assert_eq!(tree.attached(button, &ROW), 0);
  assert_eq!(tree.attached(button, &LABEL), "none");
  assert_eq!(tree.attached_values.count(button), 0);
}

// A set value reads back, and the property is created once.
#[test]
fn set_value_reads_back() {
  let (mut tree, button) = clean_button();

  tree.attached_property(button, &ROW).set(3);

  assert_eq!(tree.attached(button, &ROW), 3);
  assert_eq!(tree.attached_property(button, &ROW).get(), 3);
  assert_eq!(tree.attached_values.count(button), 1);
}

// A local value overrides a style value, which shows again when the local value is cleared.
#[test]
fn local_overrides_style() {
  let (mut tree, button) = clean_button();
  let row = tree.attached_property(button, &ROW);

  row.set_at(Precedence::StyleSetter, 2);
  row.set(1);
  assert_eq!(tree.attached(button, &ROW), 1);

  row.clear(Precedence::Local);
  assert_eq!(tree.attached(button, &ROW), 2);
}

// Two attached properties on one node are independent.
#[test]
fn kinds_are_independent() {
  let (mut tree, button) = clean_button();

  tree.attached_property(button, &ROW).set(4);
  tree.attached_property(button, &LABEL).set("ok".to_owned());

  assert_eq!(tree.attached(button, &ROW), 4);
  assert_eq!(tree.attached(button, &LABEL), "ok");
}

// A change marks the node dirty with the key's stages, like a field.
#[test]
fn change_marks_the_node_dirty() {
  let (mut tree, button) = clean_button();

  tree.attached_property(button, &LABEL).set("ok".to_owned());

  assert_eq!(tree.dirty(button), Affects::RENDER);
}

// The values go with their node: a node that later takes the slot starts at the defaults.
#[test]
fn values_go_with_their_node() {
  let (mut tree, button) = clean_button();
  tree.attached_property(button, &ROW).set(5);

  tree.free(button);
  let next = tree.alloc(Box::new(Button));

  assert_eq!(tree.attached(next, &ROW), 0);
}
