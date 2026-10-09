//! The UI thread's task queue.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::marker::PhantomData;

type Task = Box<dyn FnOnce()>;

thread_local! {
  static QUEUE: RefCell<VecDeque<Task>> = const { RefCell::new(VecDeque::new()) };
}

/// The queue of work waiting to run on the UI thread.
///
/// `Dispatcher` has no instances: its functions work on a queue that belongs to the current thread,
/// so any code on the UI thread can post work, including code running inside a task.
#[derive(Debug)]
pub struct Dispatcher(PhantomData<*const ()>);

impl Dispatcher {
  /// Adds `task` to the end of the current thread's queue.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::dispatcher::Dispatcher;
  ///
  /// assert!(Dispatcher::is_idle());
  /// Dispatcher::post_local(|| println!("later, on the UI thread"));
  /// assert!(!Dispatcher::is_idle());
  /// ```
  pub fn post_local(task: impl FnOnce() + 'static) {
    QUEUE.with(|queue| queue.borrow_mut().push_back(Box::new(task)));
  }

  /// Returns `true` if the current thread's queue holds no task.
  #[must_use]
  pub fn is_idle() -> bool {
    QUEUE.with(|queue| queue.borrow().is_empty())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use static_assertions::assert_not_impl_any;

  assert_not_impl_any!(Dispatcher: Send, Sync);

  // Posting a task makes the queue busy.
  #[test]
  fn posting_makes_the_queue_busy() {
    assert!(Dispatcher::is_idle());

    Dispatcher::post_local(|| {});

    assert!(!Dispatcher::is_idle());
  }
}
