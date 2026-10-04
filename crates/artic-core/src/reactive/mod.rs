mod runtime;
mod signal;
#[cfg(test)]
mod tests;

use crate::reactive::runtime::{ObserverId, Runtime};
pub use signal::Signal;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(crate) fn run_tracked(observer_id: ObserverId, action: impl FnOnce()) {
  Runtime::push_observer(observer_id);
  let action_result = catch_unwind(AssertUnwindSafe(action));
  Runtime::pop_observer();
  if let Err(payload) = action_result {
    resume_unwind(payload);
  }
}
