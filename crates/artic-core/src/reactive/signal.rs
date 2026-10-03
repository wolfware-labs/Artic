use crate::reactive::runtime::{Runtime, SignalId};
use std::marker::PhantomData;

#[derive(Debug)]
pub struct Signal<T>(SignalId, PhantomData<*const T>)
where
  T: 'static;

impl<T> Clone for Signal<T> {
  fn clone(&self) -> Self {
    Self(self.0, PhantomData)
  }
}

impl<T> Copy for Signal<T> {}

impl<T> Signal<T> {
  pub fn new(initial_value: T) -> Signal<T> {
    Signal(Runtime::alloc_value(initial_value), PhantomData)
  }

  pub fn with(&self, action: impl FnOnce(&T)) {
    Runtime::with_value(self.0, action);
  }

  pub fn set(&self, new_value: T) {
    Runtime::set_value(self.0, new_value)
  }

  pub fn update(&self, action: impl FnOnce(&mut T)) {
    Runtime::with_mut_value(self.0, action);
  }
}

impl<T> Signal<T>
where
  T: Clone,
{
  pub fn get(&self) -> T {
    Runtime::get_value(self.0)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use static_assertions::{assert_impl_all, assert_not_impl_any};

  assert_not_impl_any!(Signal<i32>: Send, Sync);
  assert_impl_all!(Signal<i32>: Copy);
}
