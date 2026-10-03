mod observer;
mod runtime;
mod signal;

use crate::reactive::observer::Observer;
use crate::reactive::runtime::Runtime;

pub use signal::Signal;

pub(crate) fn run_tracked(observer: Observer, action: impl FnOnce()) {
  Runtime::push_observer(observer);
  action();
  Runtime::pop_observer();
}
