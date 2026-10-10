//! Which frame stages each node needs redone.
//!
//! A node's dirty flags are an [`Affects`]: layout, render, or both. A new node starts with both. A
//! property change adds the stages its key affects, through flags the node shares with its
//! properties; an edit to a node's visual children adds layout. The frame stages clear what they
//! redo: layout clears [`Affects::LAYOUT`], rendering clears [`Affects::RENDER`].

use crate::arena::SlotMap;
use crate::property::Affects;
use crate::tree::{NodeId, UiTree};
use std::cell::Cell;
use std::rc::Rc;

/// Each node's dirty flags, shared with its properties.
#[derive(Debug, Default)]
pub(super) struct DirtyFlags {
  flags: SlotMap<Rc<Cell<Affects>>>,
}

impl DirtyFlags {
  pub(super) const fn new() -> Self {
    Self { flags: SlotMap::new() }
  }

  /// Gives `node` new flags with layout and rendering dirty, and returns them for its properties to
  /// share.
  pub(super) fn insert(&mut self, node: NodeId) -> Rc<Cell<Affects>> {
    let flags = Rc::new(Cell::new(Affects::LAYOUT | Affects::RENDER));
    self.flags.insert(node.0, Rc::clone(&flags));
    flags
  }

  /// Drops `node`'s flags. Its properties keep their own reference to them.
  pub(super) fn remove(&mut self, node: NodeId) {
    self.flags.remove(node.0);
  }

  pub(super) fn get(&self, node: NodeId) -> Affects {
    self.flags.get(node.0).map_or(Affects::NONE, |flags| flags.get())
  }

  pub(super) fn mark(&self, node: NodeId, stages: Affects) {
    if let Some(flags) = self.flags.get(node.0) {
      flags.set(flags.get() | stages);
    }
  }

  pub(super) fn clear(&self, node: NodeId, stages: Affects) {
    if let Some(flags) = self.flags.get(node.0) {
      flags.set(flags.get().without(stages));
    }
  }

  pub(super) fn shared(&self, node: NodeId) -> Rc<Cell<Affects>> {
    Rc::clone(self.flags.get(node.0).expect("a live node has dirty flags"))
  }
}

impl UiTree {
  /// Returns the frame stages `id` needs redone, or none if `id` is stale.
  ///
  /// A new node starts with layout and rendering dirty. A change to the effective value of one of
  /// its properties adds the stages its key [`affects`](crate::property::PropertyKey::affects).
  #[must_use]
  pub fn dirty(&self, id: NodeId) -> Affects {
    self.dirty_flags.get(id)
  }

  /// Adds `stages` to the frame stages `id` needs redone.
  ///
  /// # Panics
  ///
  /// Panics if `id` is stale.
  #[track_caller]
  pub fn mark_dirty(&mut self, id: NodeId, stages: Affects) {
    self.check(id);
    self.dirty_flags.mark(id, stages);
  }

  /// Removes `stages` from the frame stages `id` needs redone, once they have been redone.
  ///
  /// # Panics
  ///
  /// Panics if `id` is stale.
  #[track_caller]
  pub fn clear_dirty(&mut self, id: NodeId, stages: Affects) {
    self.check(id);
    self.dirty_flags.clear(id, stages);
  }
}

#[cfg(test)]
mod tests {
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
}
