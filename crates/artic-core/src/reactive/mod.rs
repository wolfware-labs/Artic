mod observer;
mod runtime;
mod signal;

use crate::reactive::observer::Observer;
use crate::reactive::runtime::Runtime;

pub use signal::Signal;

pub(crate) fn run_tracked(observer: Observer, action: impl FnOnce()) {
  let observer_id = Runtime::alloc_observer(observer);
  Runtime::push_observer(observer_id);
  action();
  Runtime::pop_observer();
}
