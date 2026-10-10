//! Attached property values, stored on the nodes they're set on.
//!
//! An attached property, such as `Grid.Row`, is declared by one type and set on nodes of others, so
//! the node has no field for it. Its value lives in the node's entry of [`AttachedValues`], keyed
//! by its [`AttachedKey`]: created at the key's default the first time it's needed, read as that
//! default without storing anything, and connected to the node's dirty flags like a field.

use crate::arena::SlotMap;
use crate::property::{AnyProperty, AttachedKey, KeyId, Property};
use crate::tree::{NodeId, UiTree};

/// The attached property values of each node that has any.
#[derive(Debug, Default)]
pub(super) struct AttachedValues {
  values: SlotMap<Vec<Box<dyn AnyProperty>>>,
}

impl AttachedValues {
  pub(super) const fn new() -> Self {
    Self { values: SlotMap::new() }
  }

  pub(super) fn find(&self, node: NodeId, key: KeyId) -> Option<&dyn AnyProperty> {
    let properties = self.values.get(node.0)?;
    properties.iter().find(|property| property.key_id() == key).map(AsRef::as_ref)
  }

  pub(super) fn insert(&mut self, node: NodeId, property: Box<dyn AnyProperty>) {
    match self.values.get_mut(node.0) {
      Some(properties) => properties.push(property),
      None => self.values.insert(node.0, vec![property]),
    }
  }

  pub(super) fn remove(&mut self, node: NodeId) {
    self.values.remove(node.0);
  }
}

impl UiTree {
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
  /// use artic_core::tree::UiTree;
  ///
  /// static ROW: AttachedKey<usize> = AttachedKey::new("Row", Affects::LAYOUT, || 0);
  ///
  /// #[derive(Debug)]
  /// struct Button;
  /// impl Object for Button {
  ///   fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  /// }
  ///
  /// let mut tree = UiTree::new();
  /// let button = tree.alloc(Box::new(Button));
  /// assert_eq!(tree.attached(button, &ROW), 0);
  ///
  /// tree.attached_property(button, &ROW).set_at(Precedence::StyleSetter, 2);
  /// tree.attached_property(button, &ROW).set(1);
  /// assert_eq!(tree.attached(button, &ROW), 1, "a local value overrides a style value");
  /// ```
  #[track_caller]
  pub fn attached_property<T>(&mut self, node: NodeId, key: &'static AttachedKey<T>) -> Property<T>
  where
    T: PartialEq + 'static,
  {
    if let Some(property) = self.stored(node, key) {
      return property;
    }
    self.check(node);
    let property = Property::new(key.key(), key.default_value());
    property.attach(self.dirty_flags.shared(node));
    self.attached_values.insert(node, Box::new(property));
    property
  }

  /// The property `node` stores for `key`, if one was created.
  #[track_caller]
  fn stored<T>(&self, node: NodeId, key: &'static AttachedKey<T>) -> Option<Property<T>>
  where
    T: PartialEq + 'static,
  {
    self.check(node);
    let property = self.attached_values.find(node, KeyId::of(key.key()))?;
    let property = property.as_any().downcast_ref::<Property<T>>();
    Some(*property.expect("a key's property has the key's value type"))
  }
}

#[cfg(test)]
mod tests {
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
    assert!(tree.attached_values.values.get(button.0).is_none());
  }

  // A set value reads back, and the property is created once.
  #[test]
  fn set_value_reads_back() {
    let (mut tree, button) = clean_button();

    tree.attached_property(button, &ROW).set(3);

    assert_eq!(tree.attached(button, &ROW), 3);
    assert_eq!(tree.attached_property(button, &ROW).get(), 3);
    assert_eq!(tree.attached_values.values.get(button.0).map(Vec::len), Some(1));
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
}
