use crate::reactive::runtime::{MemoId, ObservableId, Runtime};
use std::marker::PhantomData;
use std::rc::Rc;

/// A value computed from signals and other memos, and remembered until one of them changes.
///
/// A memo runs its closure lazily: not when it is created and not when a dependency changes, but on
/// the first read after a change. Every signal or memo the closure reads becomes a dependency, and
/// each run records them afresh, so a value read only on a branch that is no longer taken stops
/// being a dependency.
///
/// Reading a memo inside a memo or an effect makes it a dependency there, just like reading a
/// [`Signal`]. When a run produces a value equal to the remembered one, the memo's readers are not
/// marked out of date: the change stops at the memo.
///
/// `Memo<T>` is a `Copy` handle: copies refer to the same memo. Like `Signal<T>`, it is neither
/// `Send` nor `Sync`.
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
pub struct Memo<TValue>(MemoId, PhantomData<*const TValue>)
where
  TValue: 'static;

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
  /// Creates a memo whose value is computed by `compute`.
  ///
  /// `compute` doesn't run here. It runs on the first read, and again on the first read after one of
  /// its dependencies changes. If it panics, the panic reaches the reader and the next read runs it
  /// again.
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
    let value_id = Runtime::alloc_signal_value(None::<TValue>);
    let memo_id = Runtime::create_memo(
      value_id,
      Rc::new(move || {
        let new_value = Some(compute());
        if Runtime::with_untracked_signal_value(value_id, |value: &Option<TValue>| *value != new_value) {
          Runtime::set_memo_value(value_id, new_value);
        }
      }),
    );
    Memo(memo_id, PhantomData)
  }
}

impl<TValue> Memo<TValue> {
  /// Calls `function` with a reference to the current value and returns its result.
  ///
  /// The value is computed first if a dependency changed since the last read. Use this instead of
  /// [`get`](Memo::get) to read part of a value, or a value that isn't `Clone`, without cloning it.
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
    Runtime::refresh_memo(self.0);
    Runtime::record_read(ObservableId::Memo(self.0));
    Runtime::with_signal_value(Runtime::memo_value(self.0), |value: &Option<TValue>| {
      function(value.as_ref().expect("a refreshed memo has a value"))
    })
  }
}

impl<TValue> Memo<TValue>
where
  TValue: Clone,
{
  /// Returns a clone of the current value, computing it first if a dependency changed since the last
  /// read.
  ///
  /// To read without cloning, use [`with`](Memo::with).
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::reactive::{Memo, Signal};
  ///
  /// let count = Signal::new(2);
  /// let double = Memo::new(move || count.get() * 2);
  /// assert_eq!(double.get(), 4);
  /// ```
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
