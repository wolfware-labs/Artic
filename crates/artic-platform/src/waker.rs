//! Waking a host's loop from another thread.

use std::fmt::{Debug, Formatter, Result};
use std::sync::Arc;

/// Wakes a host's loop from any thread, so it calls
/// [`AppHandler::woken`](crate::AppHandler::woken) there.
///
/// A host builds one in its loop and hands it to [`AppHandler::started`](crate::AppHandler::started),
/// and another thread wakes the loop through it, such as when it posts work for the UI thread.
/// The hook is the host's: `artic-winit` sends an event through winit's loop proxy.
#[derive(Clone)]
pub struct Waker(Arc<dyn Fn() + Send + Sync>);

impl Waker {
  /// A waker that runs `wake` on each [`wake`](Waker::wake), from whichever thread calls it.
  pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Waker {
    Waker(Arc::new(wake))
  }

  /// Wakes the loop.
  pub fn wake(&self) {
    (self.0)();
  }
}

impl Debug for Waker {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
    formatter.write_str("Waker")
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use static_assertions::assert_impl_all;
  use std::sync::atomic::{AtomicUsize, Ordering};

  assert_impl_all!(Waker: Send, Sync, Clone);

  // A waker cloned to another thread runs its hook there.
  #[test]
  fn waker_is_send_and_runs_its_hook() {
    let count = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&count);
    let waker = Waker::new(move || {
      counted.fetch_add(1, Ordering::SeqCst);
    });

    let clone = waker.clone();
    std::thread::spawn(move || clone.wake()).join().unwrap();

    assert_eq!(count.load(Ordering::SeqCst), 1);
  }
}
