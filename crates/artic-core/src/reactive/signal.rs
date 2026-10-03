use crate::reactive::runtime::{Runtime, SignalId};
use std::marker::PhantomData;

#[derive(Debug)]
pub struct Signal<T>(SignalId, PhantomData<*const T>)
where
  T: 'static;

impl<T> Signal<T> {
  pub fn new(initial_value: T) -> Signal<T> {
    Signal(Runtime::alloc_value(initial_value), PhantomData)
  }

  pub fn get(&self) -> T
  where
    T: Clone,
  {
    Runtime::get_value(self.0).expect("the reactive runtime can't return the value for the current signal")
  }

  pub fn set(&self, new_value: T) {
    Runtime::set_value(self.0, new_value)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use static_assertions::assert_not_impl_any;

  assert_not_impl_any!(Signal<i32>: Send, Sync);
}
