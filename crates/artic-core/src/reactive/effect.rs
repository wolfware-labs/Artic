use crate::reactive::runtime::Runtime;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

/// A closure that runs again whenever a signal or memo it read changes.
///
/// An effect runs once when it is created, recording every signal and memo it reads. After one of
/// them changes, the effect doesn't run right away: it runs at the next flush point, which is
/// [`flush_effects`] or the end of the outermost [`batch`]. Like a memo, it records its dependencies
/// afresh on each run.
///
/// Effects are where reactive values reach the outside world: they update widgets, write logs or
/// mark layout as dirty. They return nothing, and their closure may keep its own mutable state
/// between runs.
///
/// `Effect` is a `Copy` handle and is neither `Send` nor `Sync`.
///
/// [`flush_effects`]: crate::reactive::flush_effects
/// [`batch`]: crate::reactive::batch
///
/// # Examples
///
/// ```
/// use artic_core::reactive::{Effect, Signal, flush_effects};
/// use std::cell::Cell;
/// use std::rc::Rc;
///
/// let count = Signal::new(1);
/// let seen = Rc::new(Cell::new(0));
/// let seen_in_effect = Rc::clone(&seen);
/// Effect::new(move || seen_in_effect.set(count.get()));
/// assert_eq!(seen.get(), 1);
///
/// count.set(2);
/// assert_eq!(seen.get(), 1, "effects wait for a flush point");
/// flush_effects();
/// assert_eq!(seen.get(), 2);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Effect(PhantomData<*const ()>);

impl Effect {
  /// Creates an effect and runs `run` once, right away.
  ///
  /// If `run` panics during that first run, the panic reaches the caller and the next flush runs it
  /// again.
  pub fn new(run: impl FnMut() + 'static) -> Effect {
    Runtime::create_effect(Rc::new(RefCell::new(run)));
    Effect(PhantomData)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use static_assertions::{assert_impl_all, assert_not_impl_any};

  assert_not_impl_any!(Effect: Send, Sync);
  assert_impl_all!(Effect: Copy);
}
