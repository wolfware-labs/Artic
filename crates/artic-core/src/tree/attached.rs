//! Attached property values, stored on the nodes they're set on.
//!
//! An attached property, such as `Grid.Row`, is declared by one type and set on nodes of others, so
//! the node has no field for it. Its value lives in the node's attached list, keyed by its
//! [`AttachedKey`]: created at the key's default the first time it's
//! needed, read as that default without storing anything, and connected to the node's dirty flags
//! like a field.

use crate::property::{AnyProperty, AttachedKey, KeyId, Property};
use crate::tree::{NodeId, TreeArena};

impl TreeArena {
  /// Returns `node`'s value of the attached property `key`, or the key's default when nothing has
  /// set it. Reading never stores anything on the node.
  ///
  /// # Panics
  ///
  /// Panics if `node` is stale.
  #[track_caller]
  #[must_use]
  pub fn attached<T>(&self, node: NodeId, key: &'static AttachedKey<T>) -> T
  where
    T: PartialEq + Clone + 'static,
  {
    self
      .stored(node, key)
      .map_or_else(|| key.default_value(), |property| property.get())
  }

  /// Returns `node`'s property for the attached property `key`, creating it at the key's default the
  /// first time. A new property is connected to the node's dirty flags like any of its fields, and
  /// its levels let a style value and a local value compete as usual.
  ///
  /// # Panics
  ///
  /// Panics if `node` is stale.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::Object;
  /// use artic_core::property::{Affects, AnyProperty, AttachedKey, Precedence};
  /// use artic_core::tree::TreeArena;
  ///
  /// static ROW: AttachedKey<usize> = AttachedKey::new("Row", Affects::LAYOUT, || 0);
  ///
  /// #[derive(Debug)]
  /// struct Button;
  /// impl Object for Button {
  ///   fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  /// }
  ///
  /// let mut arena = TreeArena::new();
  /// let button = arena.alloc(Box::new(Button));
  /// assert_eq!(arena.attached(button, &ROW), 0);
  ///
  /// arena.attached_property(button, &ROW).set_at(Precedence::StyleSetter, 2);
  /// arena.attached_property(button, &ROW).set(1);
  /// assert_eq!(arena.attached(button, &ROW), 1, "a local value overrides a style value");
  /// ```
  #[track_caller]
  pub fn attached_property<T>(&mut self, node: NodeId, key: &'static AttachedKey<T>) -> Property<T>
  where
    T: PartialEq + 'static,
  {
    if let Some(property) = self.stored(node, key) {
      return property;
    }
    let property = Property::new(key.key(), key.default_value());
    property.attach(self.dirty_flags.shared(node.slot()));
    self.node_mut(node).attached.push(Box::new(property));
    property
  }

  /// The property `node` stores for `key`, if one was created.
  #[track_caller]
  fn stored<T>(&self, node: NodeId, key: &'static AttachedKey<T>) -> Option<Property<T>>
  where
    T: PartialEq + 'static,
  {
    let id = KeyId::of(key.key());
    let property = self.node(node).attached.iter().find(|property| property.key_id() == id)?;
    let property = property.as_any().downcast_ref::<Property<T>>();
    Some(*property.expect("a key's property has the key's value type"))
  }
}

#[cfg(test)]
mod tests {
  use crate::Object;
  use crate::property::{Affects, AnyProperty, AttachedKey, Precedence};
  use crate::tree::{NodeId, TreeArena};
  use pretty_assertions::assert_eq;

  static ROW: AttachedKey<usize> = AttachedKey::new("Row", Affects::LAYOUT, || 0);
  static LABEL: AttachedKey<String> = AttachedKey::new("Label", Affects::RENDER, || "none".to_owned());

  #[derive(Debug)]
  struct Button;

  impl Object for Button {
    fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  }

  /// An arena with one button whose dirty flags are already cleared.
  fn clean_button() -> (TreeArena, NodeId) {
    let mut arena = TreeArena::new();
    let button = arena.alloc(Box::new(Button));
    arena.clear_dirty(button, Affects::LAYOUT | Affects::RENDER);
    (arena, button)
  }

  // Reading an attached property nothing set returns its default and stores nothing.
  #[test]
  fn unset_value_is_the_default_without_storing() {
    let (arena, button) = clean_button();

    assert_eq!(arena.attached(button, &ROW), 0);
    assert_eq!(arena.attached(button, &LABEL), "none");
    assert!(arena.node(button).attached.is_empty());
  }

  // A set value reads back, and the property is created once.
  #[test]
  fn set_value_reads_back() {
    let (mut arena, button) = clean_button();

    arena.attached_property(button, &ROW).set(3);

    assert_eq!(arena.attached(button, &ROW), 3);
    assert_eq!(arena.attached_property(button, &ROW).get(), 3);
    assert_eq!(arena.node(button).attached.len(), 1);
  }

  // A local value overrides a style value, which shows again when the local value is cleared.
  #[test]
  fn local_overrides_style() {
    let (mut arena, button) = clean_button();
    let row = arena.attached_property(button, &ROW);

    row.set_at(Precedence::StyleSetter, 2);
    row.set(1);
    assert_eq!(arena.attached(button, &ROW), 1);

    row.clear(Precedence::Local);
    assert_eq!(arena.attached(button, &ROW), 2);
  }

  // Two attached properties on one node are independent.
  #[test]
  fn kinds_are_independent() {
    let (mut arena, button) = clean_button();

    arena.attached_property(button, &ROW).set(4);
    arena.attached_property(button, &LABEL).set("ok".to_owned());

    assert_eq!(arena.attached(button, &ROW), 4);
    assert_eq!(arena.attached(button, &LABEL), "ok");
  }

  // A change marks the node dirty with the key's stages, like a field.
  #[test]
  fn change_marks_the_node_dirty() {
    let (mut arena, button) = clean_button();

    arena.attached_property(button, &LABEL).set("ok".to_owned());

    assert_eq!(arena.dirty(button), Affects::RENDER);
  }

  // The values go with their node: a node that later takes the slot starts at the defaults.
  #[test]
  fn values_go_with_their_node() {
    let (mut arena, button) = clean_button();
    arena.attached_property(button, &ROW).set(5);

    arena.free(button);
    let next = arena.alloc(Box::new(Button));

    assert_eq!(arena.attached(next, &ROW), 0);
  }
}
