//! Properties: reactive values that several sources can write at once, ranked by precedence.
//!
//! An animation, app code, a binding, a style, inheritance and a default can all hold a value for
//! the same [`Property`]. Each writes its own [`Precedence`] level, and the highest level holding a
//! value wins. When a source stops having an opinion it clears its level, and the next one shows
//! through.
//!
//! ```
//! use artic_core::property::{Affects, Precedence, Property, PropertyKey};
//! use artic_core::reactive::{effect, flush_effects};
//! use std::cell::Cell;
//! use std::rc::Rc;
//!
//! static BACKGROUND: PropertyKey<&str> = PropertyKey::new("Background", Affects::RENDER, false);
//!
//! let background = Property::new(&BACKGROUND, "white");
//! let painted = Rc::new(Cell::new(""));
//! let painted_in_effect = Rc::clone(&painted);
//! effect(move || painted_in_effect.set(background.get()));
//!
//! background.set_at(Precedence::StyleSetter, "blue");
//! background.set_at(Precedence::Animation, "lightblue"); // hover starts
//! flush_effects();
//! assert_eq!(painted.get(), "lightblue");
//!
//! background.clear(Precedence::Animation); // hover ends
//! flush_effects();
//! assert_eq!(painted.get(), "blue");
//! ```

mod affects;
mod any;
mod key;
mod levels;

use crate::reactive::Signal;
pub use affects::Affects;
pub use any::{AnyProperty, KeyId};
pub use key::PropertyKey;
use levels::Levels;

/// Who supplied a property's value, from lowest to highest priority.
///
/// The ordering follows priority, so a higher level compares greater:
/// `Animation` > `Local` > `Binding` > `StyleSetter` > `Inherited` > `Default`.
///
/// # Examples
///
/// ```
/// use artic_core::property::Precedence;
///
/// assert!(Precedence::Animation > Precedence::Local);
/// assert!(Precedence::StyleSetter > Precedence::Default);
/// ```
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Precedence {
  /// The value a property has when no other level is set.
  Default,
  /// A value inherited from an ancestor node.
  Inherited,
  /// A value set by a matching style.
  StyleSetter,
  /// A value written by a data binding.
  Binding,
  /// A value set directly: in markup, or with `set` in code.
  Local,
  /// A value written by a running animation.
  Animation,
}

/// A reactive value that several sources can write at once, each at its own [`Precedence`] level.
///
/// Every level holds its own value, or none. The property's *effective* value comes from the highest
/// level that holds one; `Default` always does. Writing a level only replaces that level's value,
/// and [`clear`](Property::clear) empties it so the next level down shows through. A style that
/// stops matching or an animation that ends clears its level, and nobody has to remember what the
/// value was before.
///
/// Reading the property through [`get`](Property::get) or [`with`](Property::with) subscribes the
/// running observer. Readers are marked dirty only when what they can see changes: the effective
/// value or the level it comes from. A write to a level below the winning one wakes nobody.
///
/// `Property<T>` is a `Copy` handle, like [`Signal`], and is neither `Send` nor `Sync`. It belongs to
/// the [`Scope`](crate::reactive::Scope) that was active when it was created.
///
/// # Examples
///
/// ```
/// use artic_core::property::{Affects, Precedence, Property, PropertyKey};
///
/// static WIDTH: PropertyKey<f64> = PropertyKey::new("Width", Affects::LAYOUT, false);
///
/// let width = Property::new(&WIDTH, 0.0);
/// width.set_at(Precedence::StyleSetter, 50.0);
/// width.set(100.0); // the Local level
/// assert_eq!(width.get(), 100.0);
///
/// width.clear(Precedence::Local);
/// assert_eq!(width.get(), 50.0);
/// ```
#[derive(Debug)]
pub struct Property<T>(Signal<Levels<T>>)
where
  T: 'static;

impl<T> Clone for Property<T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T> Copy for Property<T> {}

impl<T> Property<T>
where
  T: PartialEq,
{
  /// Creates a property of the kind `key` whose `Default` level holds `default` and whose other
  /// levels are empty.
  #[track_caller]
  pub fn new(key: &'static PropertyKey<T>, default: T) -> Property<T> {
    Property(Signal::new(Levels::new(key, default)))
  }

  /// Returns the key that identifies this property's kind and holds its metadata.
  ///
  /// The key never changes, so reading it doesn't subscribe the running observer.
  pub fn key(&self) -> &'static PropertyKey<T> {
    self.0.with_untracked(Levels::key)
  }

  /// Calls `function` with a reference to the effective value and returns its result.
  ///
  /// Use this instead of [`get`](Property::get) to read a value that isn't `Clone`, or part of one,
  /// without cloning it.
  pub fn with<TResult>(&self, function: impl FnOnce(&T) -> TResult) -> TResult {
    self.0.with(|levels| function(levels.effective()))
  }

  /// Returns the level that supplies the effective value.
  ///
  /// Like [`get`](Property::get), this subscribes the running observer. Clearing the winning level
  /// changes the source even when the next level holds an equal value, and readers are marked dirty
  /// then too.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::property::{Affects, Precedence, Property, PropertyKey};
  ///
  /// static WIDTH: PropertyKey<f64> = PropertyKey::new("Width", Affects::LAYOUT, false);
  ///
  /// let width = Property::new(&WIDTH, 0.0);
  /// assert_eq!(width.effective_source(), Precedence::Default);
  ///
  /// width.set_at(Precedence::Binding, 80.0);
  /// assert_eq!(width.effective_source(), Precedence::Binding);
  /// ```
  pub fn effective_source(&self) -> Precedence {
    self.0.with(|levels| levels.effective_source())
  }

  /// Sets the `Local` level, the level for values set in code or in markup.
  pub fn set(&self, value: T) {
    self.set_at(Precedence::Local, value);
  }

  /// Sets `level` to `value`.
  ///
  /// If a higher level holds a value, the effective value doesn't change and readers aren't marked
  /// dirty; `value` shows once the levels above it are cleared. Setting a level to the value it
  /// already holds does nothing.
  pub fn set_at(&self, level: Precedence, value: T) {
    self.0.update_with(|levels| levels.set(level, value));
  }

  /// Empties `level`, so the next level down that holds a value becomes effective.
  ///
  /// Clearing a level that holds nothing does nothing.
  ///
  /// # Panics
  ///
  /// Panics if `level` is [`Precedence::Default`]: a property always has a default value.
  pub fn clear(&self, level: Precedence) {
    assert_ne!(level, Precedence::Default, "the Default level can't be cleared");
    self.0.update_with(|levels| levels.clear(level));
  }
}

impl<T> Property<T>
where
  T: PartialEq + Clone,
{
  /// Returns a clone of the effective value.
  ///
  /// To read without cloning, use [`with`](Property::with).
  pub fn get(&self) -> T {
    self.with(T::clone)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::reactive::{effect, flush_effects};
  use pretty_assertions::assert_eq;
  use static_assertions::{assert_impl_all, assert_not_impl_any};
  use std::cell::Cell;
  use std::rc::Rc;

  assert_not_impl_any!(Property<i32>: Send, Sync);
  assert_impl_all!(Property<String>: Copy);

  static TEST: PropertyKey<i32> = PropertyKey::new("Test", Affects::LAYOUT, false);

  /// Every level, lowest first.
  const LEVELS: [Precedence; 6] = [
    Precedence::Default,
    Precedence::Inherited,
    Precedence::StyleSetter,
    Precedence::Binding,
    Precedence::Local,
    Precedence::Animation,
  ];

  // Sorting the levels puts them in priority order, lowest first.
  #[test]
  fn precedence_orders_levels_by_priority() {
    let mut levels = [
      Precedence::Local,
      Precedence::Default,
      Precedence::Animation,
      Precedence::StyleSetter,
      Precedence::Inherited,
      Precedence::Binding,
    ];

    levels.sort();

    assert_eq!(
      levels,
      [
        Precedence::Default,
        Precedence::Inherited,
        Precedence::StyleSetter,
        Precedence::Binding,
        Precedence::Local,
        Precedence::Animation,
      ]
    );
  }

  // For each pair of adjacent levels, the higher one's value wins over the lower one's.
  #[test]
  fn higher_level_overrides_lower() {
    for pair in LEVELS.windows(2) {
      let (lower, higher) = (pair[0], pair[1]);
      let property = Property::new(&TEST, -1);

      property.set_at(lower, 1);
      property.set_at(higher, 2);

      assert_eq!(property.get(), 2, "{higher:?} over {lower:?}");
    }
  }

  // Clearing the winning level reveals the next level down that holds a value, skipping empty
  // ones.
  #[test]
  fn clear_reverts_to_next_level() {
    let property = Property::new(&TEST, 0);
    property.set_at(Precedence::StyleSetter, 50);
    property.set_at(Precedence::Animation, 200);

    property.clear(Precedence::Animation);
    assert_eq!(property.get(), 50);

    property.clear(Precedence::StyleSetter);
    assert_eq!(property.get(), 0);
  }

  // An effect reading the property reruns once per change of the effective value, and not for a
  // write to a level below the winning one.
  #[test]
  fn effect_on_property_runs_once_per_effective_change() {
    let width = Property::new(&TEST, 0);
    let runs = Rc::new(Cell::new(0));
    let runs_in_effect = Rc::clone(&runs);
    effect(move || {
      runs_in_effect.set(runs_in_effect.get() + 1);
      width.get();
    });

    width.set(100);
    flush_effects();
    assert_eq!(runs.get(), 2, "set at Local changes the effective value");

    width.set_at(Precedence::StyleSetter, 50);
    flush_effects();
    assert_eq!(runs.get(), 2, "Local still wins, so nothing visible changed");

    width.clear(Precedence::Local);
    flush_effects();
    assert_eq!(runs.get(), 3, "the style value shows through");
    assert_eq!(width.get(), 50);
  }

  // Setting a level to the value it already holds doesn't rerun readers.
  #[test]
  fn setting_the_same_value_does_not_rerun_readers() {
    let width = Property::new(&TEST, 0);
    width.set(10);
    let runs = Rc::new(Cell::new(0));
    let runs_in_effect = Rc::clone(&runs);
    effect(move || {
      runs_in_effect.set(runs_in_effect.get() + 1);
      width.get();
    });

    width.set(10);
    flush_effects();

    assert_eq!(runs.get(), 1, "only the run on creation");
  }

  // Clearing the Default level panics: a property always has a default value.
  #[test]
  #[should_panic(expected = "the Default level can't be cleared")]
  fn clear_default_is_rejected() {
    Property::new(&TEST, 0).clear(Precedence::Default);
  }

  // `effective_source` names the highest level that holds a value, following sets and clears.
  #[test]
  fn effective_source_reports_level() {
    let property = Property::new(&TEST, 0);
    assert_eq!(property.effective_source(), Precedence::Default);

    for level in &LEVELS[1..] {
      property.set_at(*level, 1);
      assert_eq!(property.effective_source(), *level);
    }
    for pair in LEVELS.windows(2).rev() {
      property.clear(pair[1]);
      assert_eq!(property.effective_source(), pair[0]);
    }
  }

  // Clearing the winning level wakes a reader of `effective_source` even when the level below holds
  // an equal value, because the source changed.
  #[test]
  fn falling_back_to_an_equal_value_still_updates_the_source() {
    let width = Property::new(&TEST, 0);
    width.set_at(Precedence::StyleSetter, 10);
    width.set(10);
    let seen = Rc::new(Cell::new(Precedence::Default));
    let seen_in_effect = Rc::clone(&seen);
    effect(move || seen_in_effect.set(width.effective_source()));

    width.clear(Precedence::Local);
    flush_effects();

    assert_eq!(seen.get(), Precedence::StyleSetter);
    assert_eq!(width.get(), 10);
  }

  // A property reports the key it was created with: the same static, so the same address.
  #[test]
  fn property_reports_its_key() {
    let property = Property::new(&TEST, 0);

    assert!(std::ptr::eq(property.key(), &TEST));
    assert_eq!(property.key().affects(), Affects::LAYOUT);
    assert!(!property.key().inherits());
  }

  // Combining `Affects` values keeps every stage, and `contains` checks each one.
  #[test]
  fn affects_combines_stages() {
    let mut affects = Affects::NONE;
    affects |= Affects::RENDER;

    assert!(affects.contains(Affects::RENDER));
    assert!(!affects.contains(Affects::LAYOUT));
    assert!((affects | Affects::LAYOUT).contains(Affects::LAYOUT | Affects::RENDER));
    assert!(Affects::NONE.is_empty() && !affects.is_empty());
  }
}
