use crate::reactive::run_tracked;
use crate::reactive::runtime::{ObserverId, Runtime, SignalId};
use std::marker::PhantomData;
use std::rc::Rc;

/// A value computed from other reactive values, cached until one of them changes.
///
/// A memo runs its closure lazily: not when it is created, and not when a dependency changes, but on
/// the next read after a change. Every signal or memo the closure reads becomes a dependency, and
/// the dependencies are recorded afresh on each run, so a value read only on some branch stops
/// being a dependency once the branch isn't taken.
///
/// Reading a memo inside an observer subscribes that observer, just like reading a [`Signal`]. When a
/// recompute produces a value equal to the cached one, the memo's own subscribers are not marked
/// out of date: the change stops at the memo.
///
/// `Memo<T>` is a `Copy` handle, and like `Signal<T>` it is neither `Send` nor `Sync`.
///
/// [`Signal`]: crate::reactive::Signal
///
/// # Examples
///
/// ```
/// use artic_core::reactive::{Memo, Signal};
///
/// let price = Signal::new(10);
/// let quantity = Signal::new(3);
/// let total = Memo::new(move || price.get() * quantity.get());
/// assert_eq!(total.get(), 30);
///
/// price.set(11);
/// assert_eq!(total.get(), 33);
/// ```
///
/// # Panics
///
/// Reading a memo from inside its own closure panics, because the memo would depend on itself.
#[derive(Debug)]
pub struct Memo<TValue>
where
  TValue: 'static,
{
  observer_id: ObserverId,
  value_id: SignalId,
  value_type: PhantomData<*const TValue>,
}

impl<TValue> Clone for Memo<TValue> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<TValue> Copy for Memo<TValue> {}

impl<TValue> Memo<TValue>
where
  TValue: PartialEq,
{
  /// Creates a memo that computes its value with `compute`.
  ///
  /// `compute` doesn't run here. It runs on the first read, and again on the first read after a
  /// dependency changes.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::reactive::{Memo, Signal};
  ///
  /// let name = Signal::new(String::from("ada"));
  /// let shout = Memo::new(move || name.with(|name| name.to_uppercase()));
  /// assert_eq!(shout.get(), "ADA");
  /// ```
  pub fn new(compute: impl Fn() -> TValue + 'static) -> Memo<TValue> {
    // The slot holds `None` until the first read computes the value.
    let (observer_id, value_id) = Runtime::create_memo(None::<TValue>, |observer_id, value_id| {
      Rc::new(move || {
        let mut new_value = None;
        run_tracked(observer_id, || new_value = Some(compute()));
        // The equality cutoff: an equal value writes nothing, so subscribers stay as they are.
        if Runtime::with_untracked_signal_value(value_id, |value: &Option<TValue>| *value != new_value) {
          Runtime::set_signal_value(value_id, new_value);
        }
      })
    });
    Memo {
      observer_id,
      value_id,
      value_type: PhantomData,
    }
  }
}

impl<TValue> Memo<TValue> {
  /// Calls `function` with a reference to the current value and returns its result.
  ///
  /// The value is recomputed first if a dependency changed since the last read. Like
  /// [`get`](Memo::get), this subscribes the running observer, if there is one.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::reactive::{Memo, Signal};
  ///
  /// let items = Signal::new(vec![3, 1, 2]);
  /// let sorted = Memo::new(move || {
  ///   let mut items = items.get();
  ///   items.sort();
  ///   items
  /// });
  /// assert_eq!(sorted.with(|sorted| sorted[0]), 1);
  /// ```
  pub fn with<TResult>(&self, function: impl FnOnce(&TValue) -> TResult) -> TResult {
    Runtime::refresh_memo(self.observer_id);
    Runtime::with_signal_value(self.value_id, |value: &Option<TValue>| {
      function(value.as_ref().expect("memo value is computed by refresh_memo"))
    })
  }
}

impl<TValue> Memo<TValue>
where
  TValue: Clone,
{
  /// Returns a clone of the current value, recomputing it first if a dependency changed.
  ///
  /// If an observer is running, it is subscribed to this memo. To read without cloning, use
  /// [`with`](Memo::with).
  pub fn get(&self) -> TValue {
    self.with(TValue::clone)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use static_assertions::{assert_impl_all, assert_not_impl_any};

  assert_not_impl_any!(Memo<i32>: Send, Sync);
  assert_impl_all!(Memo<i32>: Copy);
}
