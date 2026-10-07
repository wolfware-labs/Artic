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
//!
//! # Derived values
//!
//! A [`Memo`] computes a value from other signals and memos and caches it. It recomputes lazily, on
//! the first read after a dependency changed, and a recompute that produces an equal value doesn't
//! disturb the memo's own readers.
//!
//! ```
//! use artic_core::reactive::{Memo, Signal};
//!
//! let count = Signal::new(2);
//! let is_even = Memo::new(move || count.get() % 2 == 0);
//! assert!(is_even.get());
//! ```
//!
//! # Skipping writes that change nothing
//!
//! [`Signal::set`] and [`Signal::update`] always mark subscribers as dirty, even when the new value
//! equals the old one, so they work for every value type. For types that implement `PartialEq`,
//! [`Signal::set_if_changed`] compares first and does nothing when the values are equal.
//!
//! The skip is a separate method because stable Rust can't give one method two behaviors depending
//! on whether `T: PartialEq` (that would need specialization). Requiring `PartialEq` on `set` itself
//! would rule out signals of types that can't be compared, such as closures or handles, and storing
//! a comparison function per signal at construction would cost every signal for a choice most
//! writes don't need. A separate method keeps `set` unrestricted and makes skipping a visible,
//! opt-in choice at the call site.
//!
//! The comparison inside `set_if_changed` reads the value without subscribing the running
//! observer, so it is still only a write.

mod memo;
mod runtime;
mod signal;
#[cfg(test)]
mod tests;

use crate::reactive::runtime::{ObserverId, Runtime};
pub use memo::Memo;
pub use signal::Signal;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(crate) fn run_tracked(observer_id: ObserverId, action: impl FnOnce()) {
  Runtime::reset_observer(observer_id);
  Runtime::push_observer(observer_id);
  let action_result = catch_unwind(AssertUnwindSafe(action));
  Runtime::pop_observer();
  if let Err(payload) = action_result {
    resume_unwind(payload);
  }
}
