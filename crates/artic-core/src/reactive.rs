//! Fine-grained reactivity: values that know who reads them.
//!
//! A [`Signal`] holds a value and remembers which observers read it. An observer is something that
//! reads signals and is marked dirty when they change; a [`Memo`] uses one to know when its value is
//! out of date. Reading a signal while an observer runs subscribes that observer to the signal, and writing the signal marks every subscribed
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
//! A [`Memo`] computes a value from signals and other memos and remembers it. It runs its closure
//! only when read after a dependency changed, and a run that produces an equal value leaves the
//! memo's own readers up to date.
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

mod effect;
mod memo;
mod runtime;
mod signal;
#[cfg(test)]
mod tests;

pub use effect::Effect;
pub use memo::Memo;
pub use signal::Signal;

/// Runs every effect whose dependencies changed since it last ran.
///
/// Effects run in the order they were created, each at most once per pass. If an effect writes a
/// signal during the flush, another pass runs, so when `flush_effects` returns every effect has
/// seen the latest values. Effects that never settle, because they keep writing each other's
/// inputs, make it panic after 100 passes.
///
/// Calling it while a flush is already running, for example from inside an effect, does nothing:
/// the running flush picks up the change. Inside a [`batch`] it does nothing either; the outermost
/// `batch` flushes when it returns.
///
/// # Panics
///
/// Panics if the effects don't settle after 100 passes. A panic inside an effect continues to the
/// caller; that effect and the effects not yet visited run again on the next flush.
pub fn flush_effects() {
  runtime::Runtime::flush_effects();
}

/// Runs `action` and then flushes effects once, so all the writes inside it cause a single flush.
///
/// Batches nest: only the outermost `batch` flushes. If `action` panics, nothing is flushed and the
/// panic continues; the pending effects run at the next flush point.
///
/// # Examples
///
/// ```
/// use artic_core::reactive::{Effect, Signal, batch};
/// use std::cell::Cell;
/// use std::rc::Rc;
///
/// let first = Signal::new(String::from("Ada"));
/// let last = Signal::new(String::from("Byron"));
/// let runs = Rc::new(Cell::new(0));
/// let runs_in_effect = Rc::clone(&runs);
/// Effect::new(move || {
///   runs_in_effect.set(runs_in_effect.get() + 1);
///   format!("{} {}", first.get(), last.get());
/// });
///
/// batch(|| {
///   first.set(String::from("Augusta Ada"));
///   last.set(String::from("King"));
/// });
/// assert_eq!(runs.get(), 2, "one run on creation, one for the whole batch");
/// ```
pub fn batch<TResult>(action: impl FnOnce() -> TResult) -> TResult {
  runtime::Runtime::batch(action)
}
