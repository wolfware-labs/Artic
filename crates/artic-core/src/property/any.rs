use crate::property::{Affects, Property, PropertyKey};
use crate::reactive::Change;
use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

/// The identity of a property kind: the address of its [`PropertyKey`] static.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct KeyId(*const ());

impl KeyId {
  /// Returns the identity of `key`.
  #[must_use]
  pub fn of<T>(key: &'static PropertyKey<T>) -> KeyId {
    KeyId(std::ptr::from_ref(key).cast())
  }
}

/// A [`Property`] seen without its value type, as [`Object::visit_properties`] hands it out.
///
/// The tree uses it to connect a node's properties to the node and to find a property by key.
/// Every `Property<T>` implements it.
///
/// [`Object::visit_properties`]: crate::Object::visit_properties
pub trait AnyProperty {
  /// The identity of the property's kind.
  fn key_id(&self) -> KeyId;

  /// Connects the property to its node's dirty flags. After this, a change to the effective value
  /// adds the key's [`Affects`] to them.
  fn attach(&self, dirty: Rc<Cell<Affects>>);

  /// The property as `Any`, to get the typed `Property<T>` back.
  fn as_any(&self) -> &dyn Any;
}

impl<T> AnyProperty for Property<T>
where
  T: PartialEq + 'static,
{
  fn key_id(&self) -> KeyId {
    KeyId::of(self.key())
  }

  fn attach(&self, dirty: Rc<Cell<Affects>>) {
    self.0.update_with(|levels| {
      levels.attach(dirty);
      Change::Hidden
    });
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}
