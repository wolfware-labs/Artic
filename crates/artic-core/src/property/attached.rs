use crate::property::{Affects, PropertyKey};
use std::fmt::{Debug, Formatter, Result};

/// The key of an attached property: a property kind one type declares and other nodes hold, such as
/// `Grid.Row` on the children of a grid.
///
/// A node doesn't have a field for it; [`TreeArena::attached_property`] creates the property on
/// the node the first time it's needed, starting at the key's default, and
/// [`TreeArena::attached`] reads the default without creating anything. Declare one `static` per
/// attached property, for the same reason as a [`PropertyKey`].
///
/// Attached properties don't inherit: an inherited value could only reach a node that already holds
/// the property.
///
/// [`TreeArena::attached`]: crate::tree::TreeArena::attached
/// [`TreeArena::attached_property`]: crate::tree::TreeArena::attached_property
///
/// # Examples
///
/// ```
/// use artic_core::property::{Affects, AttachedKey};
///
/// static ROW: AttachedKey<usize> = AttachedKey::new("Row", Affects::LAYOUT, || 0);
///
/// assert_eq!(ROW.key().name(), "Row");
/// assert_eq!(ROW.default_value(), 0);
/// ```
pub struct AttachedKey<T> {
  key: PropertyKey<T>,
  default: fn() -> T,
}

impl<T> AttachedKey<T> {
  /// Creates the key for an attached property named `name`, whose changes redo `affects`, and whose
  /// value is `default()` on a node that doesn't set it.
  #[must_use]
  pub const fn new(name: &'static str, affects: Affects, default: fn() -> T) -> Self {
    Self {
      key: PropertyKey::new(name, affects, false),
      default,
    }
  }

  /// Returns the property key the attached property is stored under.
  #[must_use]
  pub const fn key(&self) -> &PropertyKey<T> {
    &self.key
  }

  /// Returns the value of a node that doesn't set the property.
  #[must_use]
  pub fn default_value(&self) -> T {
    (self.default)()
  }
}

impl<T> Debug for AttachedKey<T> {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
    formatter
      .debug_struct("AttachedKey")
      .field("key", &self.key)
      .finish_non_exhaustive()
  }
}
