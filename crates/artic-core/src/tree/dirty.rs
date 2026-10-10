//! Which frame stages each node needs redone.
//!
//! A node's dirty flags are an [`Affects`]: layout, render, or both. A new node starts with both. A
//! property change adds the stages its key affects, through flags the node shares with its
//! properties; an edit to a node's visual children adds layout. The frame stages clear what they
//! redo: layout clears [`Affects::LAYOUT`], rendering clears [`Affects::RENDER`].

use crate::property::Affects;
use crate::tree::{NodeId, TreeArena};

impl TreeArena {
  /// Returns the frame stages `id` needs redone, or none if `id` is stale.
  ///
  /// A new node starts with layout and rendering dirty. A change to the effective value of one of
  /// its properties adds the stages its key [`affects`](crate::property::PropertyKey::affects).
  #[must_use]
  pub fn dirty(&self, id: NodeId) -> Affects {
    self.inner.get(id.0).map_or(Affects::NONE, |node| node.dirty.get())
  }

  /// Adds `stages` to the frame stages `id` needs redone.
  ///
  /// # Panics
  ///
  /// Panics if `id` is stale.
  #[track_caller]
  pub fn mark_dirty(&mut self, id: NodeId, stages: Affects) {
    let dirty = &self.node(id).dirty;
    dirty.set(dirty.get() | stages);
  }

  /// Removes `stages` from the frame stages `id` needs redone, once they have been redone.
  ///
  /// # Panics
  ///
  /// Panics if `id` is stale.
  #[track_caller]
  pub fn clear_dirty(&mut self, id: NodeId, stages: Affects) {
    let dirty = &self.node(id).dirty;
    dirty.set(dirty.get().without(stages));
  }
}

#[cfg(test)]
mod tests {
  use crate::Object;
  use crate::property::{Affects, AnyProperty, Precedence, Property, PropertyKey};
  use crate::tree::{NodeId, TreeArena};
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
  fn clean_label(arena: &mut TreeArena) -> (NodeId, Property<i32>, Property<i32>) {
    let font_size = Property::new(&FONT_SIZE, 12);
    let foreground = Property::new(&FOREGROUND, 0);
    let id = arena.alloc(Box::new(Label { font_size, foreground }));
    arena.clear_dirty(id, Affects::LAYOUT | Affects::RENDER);
    (id, font_size, foreground)
  }

  // A new node needs layout and rendering.
  #[test]
  fn new_node_starts_dirty() {
    let mut arena = TreeArena::new();
    let id = arena.alloc(Box::new(Label {
      font_size: Property::new(&FONT_SIZE, 12),
      foreground: Property::new(&FOREGROUND, 0),
    }));

    assert_eq!(arena.dirty(id), Affects::LAYOUT | Affects::RENDER);
  }

  // Changing a property whose key affects layout marks its node layout-dirty.
  #[test]
  fn layout_affecting_change_sets_measure_dirty() {
    let mut arena = TreeArena::new();
    let (id, font_size, _) = clean_label(&mut arena);

    font_size.set(24);

    assert_eq!(arena.dirty(id), Affects::LAYOUT);
  }

  // Changing a property whose key affects only rendering marks the node render-dirty, not
  // layout-dirty.
  #[test]
  fn render_only_change_does_not_set_measure_dirty() {
    let mut arena = TreeArena::new();
    let (id, _, foreground) = clean_label(&mut arena);

    foreground.set(0xff0000);

    assert_eq!(arena.dirty(id), Affects::RENDER);
  }

  // A write that doesn't change the effective value marks nothing.
  #[test]
  fn hidden_write_marks_nothing() {
    let mut arena = TreeArena::new();
    let (id, font_size, _) = clean_label(&mut arena);
    font_size.set(24);
    arena.clear_dirty(id, Affects::LAYOUT);

    font_size.set_at(Precedence::StyleSetter, 30);
    font_size.set(24);

    assert_eq!(arena.dirty(id), Affects::NONE);
  }

  // `mark_dirty` adds stages and `clear_dirty` removes only the ones given.
  #[test]
  fn mark_and_clear_dirty_change_only_the_given_stages() {
    let mut arena = TreeArena::new();
    let (id, _, _) = clean_label(&mut arena);

    arena.mark_dirty(id, Affects::LAYOUT | Affects::RENDER);
    arena.clear_dirty(id, Affects::LAYOUT);

    assert_eq!(arena.dirty(id), Affects::RENDER);
  }
}
