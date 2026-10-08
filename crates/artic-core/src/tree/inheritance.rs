use crate::property::AnyProperty;
use crate::tree::{NodeId, Tree, TreeArena};
use std::iter::once;

impl TreeArena {
  pub(super) fn resolve_inheritance(&self, root: NodeId) {
    let subtree: Vec<NodeId> = once(root).chain(self.descendants(Tree::Logical, root)).collect();
    for node in subtree {
      self.node(node).object.visit_properties(&mut |property| {
        if property.inherits() {
          self.inherit_from_nearest_ancestor(node, property);
        }
      });
    }
  }

  fn inherit_from_nearest_ancestor(&self, node: NodeId, property: &dyn AnyProperty) {
    for ancestor in self.ancestors(Tree::Logical, node) {
      let mut found = false;
      self.node(ancestor).object.visit_properties(&mut |candidate| {
        if !found && candidate.key_id() == property.key_id() {
          property.inherit_from(Some(candidate));
          found = true;
        }
      });
      if found {
        return;
      }
    }
    property.inherit_from(None);
  }
}

#[cfg(test)]
mod tests {
  use crate::Object;
  use crate::property::{Affects, AnyProperty, Precedence, Property, PropertyKey};
  use crate::tree::{NodeId, Tree, TreeArena};
  use pretty_assertions::assert_eq;

  static FONT_SIZE: PropertyKey<i32> = PropertyKey::new("FontSize", Affects::LAYOUT, true);
  static WIDTH: PropertyKey<i32> = PropertyKey::new("Width", Affects::LAYOUT, false);

  /// A root node with an inheritable font size.
  #[derive(Debug)]
  struct Window {
    font_size: Property<i32>,
  }

  impl Object for Window {
    fn visit_properties(&self, visit: &mut dyn FnMut(&dyn AnyProperty)) {
      visit(&self.font_size);
    }
  }

  /// A container without a font size, which inheritance passes through.
  #[derive(Debug)]
  struct StackPanel;

  impl Object for StackPanel {}

  /// A leaf with an inheritable font size and a width that doesn't inherit.
  #[derive(Debug)]
  struct TextBlock {
    font_size: Property<i32>,
    width: Property<i32>,
  }

  impl Object for TextBlock {
    fn visit_properties(&self, visit: &mut dyn FnMut(&dyn AnyProperty)) {
      visit(&self.font_size);
      visit(&self.width);
    }
  }

  /// Allocates a window with `font_size` set locally.
  fn new_window(arena: &mut TreeArena, font_size: i32) -> (NodeId, Property<i32>) {
    let property = Property::new(&FONT_SIZE, 0);
    property.set(font_size);
    (arena.alloc(Box::new(Window { font_size: property })), property)
  }

  /// Allocates a text block with nothing set, and returns its font size and width.
  fn new_text_block(arena: &mut TreeArena) -> (NodeId, Property<i32>, Property<i32>) {
    let font_size = Property::new(&FONT_SIZE, 10);
    let width = Property::new(&WIDTH, 100);
    (arena.alloc(Box::new(TextBlock { font_size, width })), font_size, width)
  }

  // An inheritable value set on a window reaches a text block two logical levels down, through a
  // container type that doesn't have the property.
  #[test]
  fn inherited_value_flows_from_logical_ancestor() {
    let mut arena = TreeArena::new();
    let (window, window_font_size) = new_window(&mut arena, 20);
    let panel = arena.alloc(Box::new(StackPanel));
    let (text, font_size, _) = new_text_block(&mut arena);

    arena.add_child(Tree::Logical, window, panel);
    arena.add_child(Tree::Logical, panel, text);

    assert_eq!(font_size.get(), 20);
    assert_eq!(font_size.effective_source(), Precedence::Inherited);
    window_font_size.set(24);
    assert_eq!(font_size.get(), 24);
  }

  // A node with its own value hides the inherited one, for itself and for everything below it.
  #[test]
  fn local_value_blocks_inheritance() {
    let mut arena = TreeArena::new();
    let (window, window_font_size) = new_window(&mut arena, 20);
    let (border, border_font_size, _) = new_text_block(&mut arena);
    let (text, font_size, _) = new_text_block(&mut arena);
    border_font_size.set(12);
    arena.add_child(Tree::Logical, window, border);
    arena.add_child(Tree::Logical, border, text);
    arena.clear_dirty(text, Affects::LAYOUT | Affects::RENDER);

    window_font_size.set(30);

    assert_eq!(font_size.get(), 12);
    assert_eq!(arena.dirty(text), Affects::NONE);
  }

  // A change on an ancestor marks dirty every node that inherits it.
  #[test]
  fn inherited_change_marks_inheriting_nodes_dirty() {
    let mut arena = TreeArena::new();
    let (window, window_font_size) = new_window(&mut arena, 20);
    let panel = arena.alloc(Box::new(StackPanel));
    let (text, _, _) = new_text_block(&mut arena);
    arena.add_child(Tree::Logical, window, panel);
    arena.add_child(Tree::Logical, panel, text);
    arena.clear_dirty(text, Affects::LAYOUT | Affects::RENDER);

    window_font_size.set(24);

    assert_eq!(arena.dirty(text), Affects::LAYOUT);
  }

  // Moving a node to another logical parent makes it inherit from its new ancestors, and marks it
  // dirty.
  #[test]
  fn reparent_updates_inherited_value() {
    let mut arena = TreeArena::new();
    let (large, _) = new_window(&mut arena, 20);
    let (small, _) = new_window(&mut arena, 12);
    let (text, font_size, _) = new_text_block(&mut arena);
    arena.add_child(Tree::Logical, large, text);
    arena.clear_dirty(text, Affects::LAYOUT | Affects::RENDER);

    arena.add_child(Tree::Logical, small, text);

    assert_eq!(font_size.get(), 12);
    assert_eq!(arena.dirty(text), Affects::LAYOUT);
  }

  // Detaching a node from the logical tree leaves it with nothing to inherit from.
  #[test]
  fn detach_stops_inheriting() {
    let mut arena = TreeArena::new();
    let (window, _) = new_window(&mut arena, 20);
    let (text, font_size, _) = new_text_block(&mut arena);
    arena.add_child(Tree::Logical, window, text);

    arena.detach(Tree::Logical, text);

    assert_eq!(font_size.get(), 10);
    assert_eq!(font_size.effective_source(), Precedence::Default);
  }

  // Inheritance follows the logical parent, not a template part that is the visual parent.
  #[test]
  fn inheritance_follows_logical_not_visual_parent() {
    let mut arena = TreeArena::new();
    let (window, _) = new_window(&mut arena, 20);
    let (template_part, _) = new_window(&mut arena, 12);
    let (text, font_size, _) = new_text_block(&mut arena);

    arena.add_child(Tree::Logical, window, text);
    arena.add_child(Tree::Visual, window, template_part);
    arena.add_child(Tree::Visual, template_part, text);

    assert_eq!(font_size.get(), 20);
  }

  // A property whose key doesn't inherit keeps its own value under an ancestor that has one.
  #[test]
  fn non_inheriting_property_is_not_linked() {
    let mut arena = TreeArena::new();
    let (parent, _, parent_width) = new_text_block(&mut arena);
    let (child, _, width) = new_text_block(&mut arena);
    parent_width.set(300);

    arena.add_child(Tree::Logical, parent, child);

    assert_eq!(width.get(), 100);
  }

  // Freeing an ancestor's subtree unlinks the freed followers, so a later change on the surviving
  // root walks nothing that was freed.
  #[test]
  fn freeing_a_subtree_unlinks_its_followers() {
    let mut arena = TreeArena::new();
    let (window, window_font_size) = new_window(&mut arena, 20);
    let panel = arena.alloc(Box::new(StackPanel));
    let (text, font_size, _) = new_text_block(&mut arena);
    arena.add_child(Tree::Logical, window, panel);
    arena.add_child(Tree::Logical, panel, text);

    arena.free_subtree(panel);
    window_font_size.set(24);

    assert_eq!(font_size.effective_source(), Precedence::Default);
    assert!(!arena.contains(text));
  }

  // Moving a container re-resolves everything below it: a text block that inherited through the
  // container now inherits from the container's new ancestors.
  #[test]
  fn reparenting_a_container_updates_its_descendants() {
    let mut arena = TreeArena::new();
    let (large, _) = new_window(&mut arena, 20);
    let (small, _) = new_window(&mut arena, 12);
    let panel = arena.alloc(Box::new(StackPanel));
    let (text, font_size, _) = new_text_block(&mut arena);
    arena.add_child(Tree::Logical, large, panel);
    arena.add_child(Tree::Logical, panel, text);

    arena.add_child(Tree::Logical, small, panel);

    assert_eq!(font_size.get(), 12);
  }
}
