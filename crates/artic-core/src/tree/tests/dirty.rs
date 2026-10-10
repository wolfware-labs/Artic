use crate::Object;
use crate::property::{Affects, AnyProperty, Precedence, Property, PropertyKey};
use crate::tree::{NodeId, UiTree};
use pretty_assertions::assert_eq;

static FONT_SIZE: PropertyKey<i32> = PropertyKey::new("FontSize", Affects::LAYOUT, false);
static FOREGROUND: PropertyKey<i32> = PropertyKey::new("Foreground", Affects::RENDER, false);

#[derive(Debug)]
struct Label {
  font_size: Property<i32>,
  foreground: Property<i32>,
}

impl Object for Label {
  fn visit_properties(&self, visit: &mut dyn FnMut(&dyn AnyProperty)) {
    visit(&self.font_size);
    visit(&self.foreground);
  }
}

/// Allocates a label, clears the dirty flags every new node starts with, and returns its id with
/// its font size and foreground properties.
fn clean_label(tree: &mut UiTree) -> (NodeId, Property<i32>, Property<i32>) {
  let font_size = Property::new(&FONT_SIZE, 12);
  let foreground = Property::new(&FOREGROUND, 0);
  let id = tree.alloc(Box::new(Label { font_size, foreground }));
  tree.clear_dirty(id, Affects::LAYOUT | Affects::RENDER);
  (id, font_size, foreground)
}

// A new node needs layout and rendering.
#[test]
fn new_node_starts_dirty() {
  let mut tree = UiTree::new();
  let id = tree.alloc(Box::new(Label {
    font_size: Property::new(&FONT_SIZE, 12),
    foreground: Property::new(&FOREGROUND, 0),
  }));

  assert_eq!(tree.dirty(id), Affects::LAYOUT | Affects::RENDER);
}

// Changing a property whose key affects layout marks its node layout-dirty.
#[test]
fn layout_affecting_change_sets_measure_dirty() {
  let mut tree = UiTree::new();
  let (id, font_size, _) = clean_label(&mut tree);

  font_size.set(24);

  assert_eq!(tree.dirty(id), Affects::LAYOUT);
}

// Changing a property whose key affects only rendering marks the node render-dirty, not
// layout-dirty.
#[test]
fn render_only_change_does_not_set_measure_dirty() {
  let mut tree = UiTree::new();
  let (id, _, foreground) = clean_label(&mut tree);

  foreground.set(0xff0000);

  assert_eq!(tree.dirty(id), Affects::RENDER);
}

// A write that doesn't change the effective value marks nothing.
#[test]
fn hidden_write_marks_nothing() {
  let mut tree = UiTree::new();
  let (id, font_size, _) = clean_label(&mut tree);
  font_size.set(24);
  tree.clear_dirty(id, Affects::LAYOUT);

  font_size.set_at(Precedence::StyleSetter, 30);
  font_size.set(24);

  assert_eq!(tree.dirty(id), Affects::NONE);
}

// `mark_dirty` adds stages and `clear_dirty` removes only the ones given.
#[test]
fn mark_and_clear_dirty_change_only_the_given_stages() {
  let mut tree = UiTree::new();
  let (id, _, _) = clean_label(&mut tree);

  tree.mark_dirty(id, Affects::LAYOUT | Affects::RENDER);
  tree.clear_dirty(id, Affects::LAYOUT);

  assert_eq!(tree.dirty(id), Affects::RENDER);
}

// A freed node's properties keep their own flags: changing one doesn't mark the node that takes
// its slot.
#[test]
fn freed_node_properties_do_not_mark_the_next_node() {
  let mut tree = UiTree::new();
  let (old, font_size, _) = clean_label(&mut tree);
  tree.free(old);
  let (next, _, _) = clean_label(&mut tree);

  font_size.set(24);

  assert_eq!(tree.dirty(next), Affects::NONE);
}
