use crate::reactive::runtime::{Runtime, SignalId};
use std::marker::PhantomData;

#[derive(Debug)]
pub struct Signal<TValue>(SignalId, PhantomData<*const TValue>)
where
  TValue: 'static;

impl<T> Clone for Signal<T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<TValue> Copy for Signal<TValue> {}

impl<TValue> Signal<TValue> {
  pub fn new(initial_value: TValue) -> Signal<TValue> {
    Signal(Runtime::alloc_signal_value(initial_value), PhantomData)
  }

  pub fn with<TResult>(&self, function: impl FnOnce(&TValue) -> TResult) -> TResult {
    Runtime::with_signal_value(self.0, function)
  }

  pub fn set(&self, new_value: TValue) {
    Runtime::set_signal_value(self.0, new_value)
  }

  pub fn update(&self, action: impl FnOnce(&mut TValue)) {
    Runtime::with_signal_mut_value(self.0, action);
  }
}

impl<T> Signal<T>
where
  T: Clone,
{
  pub fn get(&self) -> T {
    Runtime::get_signal_value(self.0)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use static_assertions::{assert_impl_all, assert_not_impl_any};

  assert_not_impl_any!(Signal<i32>: Send, Sync);
  assert_impl_all!(Signal<i32>: Copy);
}
