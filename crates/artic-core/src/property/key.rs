use crate::property::Affects;
use std::fmt::{Debug, Formatter, Result};
use std::marker::PhantomData;

/// The identity and metadata of one kind of property, shared by every node type that has it.
///
/// Declare one `static` per property kind. Two properties are the same kind when they point at the
/// same `static`, even on different node types: a `FontSize` on a window and a `FontSize` on a text
/// block both use the same key, which is how one inherits from the other. The key also fixes the
/// value type, so a lookup by key can't return a property of another type.
///
/// It must be a `static`, not a `const`: a `const` is copied into each place that uses it and has no
/// single address to compare.
///
/// # Examples
///
/// ```
/// use artic_core::property::{Affects, PropertyKey};
///
/// static FONT_SIZE: PropertyKey<f32> = PropertyKey::new("FontSize", Affects::LAYOUT, true);
///
/// assert_eq!(FONT_SIZE.name(), "FontSize");
/// assert!(FONT_SIZE.inherits());
/// ```
pub struct PropertyKey<T> {
  name: &'static str,
  affects: Affects,
  inherits: bool,
  value_type: PhantomData<fn() -> T>,
}

impl<T> PropertyKey<T> {
  /// Creates the key for a property kind named `name`.
  ///
  /// `affects` lists the frame stages a change to the property's effective value has to redo.
  /// `inherits` makes the property take its value from the nearest logical ancestor that has the
  /// same key, when nothing above the `Inherited` level is set.
  #[must_use]
  pub const fn new(name: &'static str, affects: Affects, inherits: bool) -> Self {
    Self {
      name,
      affects,
      inherits,
      value_type: PhantomData,
    }
  }

  /// The property's name, as markup spells it.
  #[must_use]
  pub const fn name(&self) -> &'static str {
    self.name
  }

  /// The frame stages a change to the property's effective value has to redo.
  #[must_use]
  pub const fn affects(&self) -> Affects {
    self.affects
  }

  /// Whether the property takes its value from a logical ancestor when nothing above the
  /// `Inherited` level is set.
  #[must_use]
  pub const fn inherits(&self) -> bool {
    self.inherits
  }
}

impl<T> Debug for PropertyKey<T> {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
    formatter
      .debug_struct("PropertyKey")
      .field("name", &self.name)
      .field("affects", &self.affects)
      .field("inherits", &self.inherits)
      .finish()
  }
}
