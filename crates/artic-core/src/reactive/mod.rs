mod observer;
mod runtime;
mod signal;

use crate::reactive::observer::Observer;
use crate::reactive::runtime::Runtime;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub use signal::Signal;

pub(crate) fn run_tracked(observer: Observer, action: impl FnOnce()) {
  let observer_id = Runtime::alloc_observer(observer);
  Runtime::push_observer(observer_id);
  let action_result = catch_unwind(AssertUnwindSafe(action));
  Runtime::pop_observer();
  if let Err(payload) = action_result {
    resume_unwind(payload);
  }
}
