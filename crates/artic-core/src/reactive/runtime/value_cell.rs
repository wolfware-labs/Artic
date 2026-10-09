use std::any::Any;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::thread;

pub struct ValueCell {
  value: RefCell<Option<Box<dyn Any>>>,
  kind: &'static str,
}

impl ValueCell {
  pub fn new(value: Box<dyn Any>, kind: &'static str) -> Self {
    Self {
      value: RefCell::new(Some(value)),
      kind,
    }
  }

  pub fn with<TValue, TResult>(&self, action: impl FnOnce(&TValue) -> TResult) -> TResult
  where
    TValue: Any,
  {
    let value = self.value.borrow();
    let Some(value) = &*value else {
      panic!("{} read while it is being updated", self.kind);
    };
    action(self.downcast(value.downcast_ref::<TValue>()))
  }

  pub fn with_mut<TValue, TResult>(&self, action: impl FnOnce(&mut TValue) -> TResult) -> thread::Result<TResult>
  where
    TValue: Any,
  {
    let taken = self.value.try_borrow_mut().ok().and_then(|mut value| value.take());
    let Some(mut value) = taken else {
      panic!("{} written while it is being read or updated", self.kind);
    };
    let action_result = catch_unwind(AssertUnwindSafe(|| action(self.downcast(value.downcast_mut::<TValue>()))));
    self.value.replace(Some(value));
    action_result
  }

  fn downcast<TValue>(&self, value: Option<TValue>) -> TValue {
    value.unwrap_or_else(|| panic!("{} type does not match with the specified type", self.kind))
  }
}
