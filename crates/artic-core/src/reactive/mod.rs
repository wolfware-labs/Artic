//! Fine-grained reactivity: values that know who reads them.
//!
//! A [`Signal`] holds a value and remembers which observers read it. An observer is a computation
//! the runtime can re-run, such as a derived value or an effect. Reading a signal while an observer
//! runs subscribes that observer to the signal, and writing the signal marks every subscribed
//! observer as dirty, meaning its last result is out of date. Nothing subscribes explicitly: the
//! reads themselves record the dependencies.
//!
//! The runtime that stores signal values is thread-local, and signals can only be used on the
//! thread that created them. A `Signal<T>` is a small `Copy` handle into that runtime, so copying it
//! never copies the value.
//!
//! ```
//! use artic_core::reactive::Signal;
//!
//! let count = Signal::new(0);
//! let copy = count;            // a second handle to the same value
//! count.set(5);
//! assert_eq!(copy.get(), 5);
//! ```

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
