// Integration tests for `Effect`, using only the public API, as app code would.

use artic_core::reactive::{Effect, Memo, Signal};
use pretty_assertions::assert_eq;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Returns a call counter and a closure that bumps it, to count how often an effect runs.
fn counter() -> (Rc<Cell<u32>>, impl Fn() + 'static) {
  let calls = Rc::new(Cell::new(0));
  let calls_in_closure = Rc::clone(&calls);
  (calls, move || calls_in_closure.set(calls_in_closure.get() + 1))
}

// Creation

// Creating an effect runs it once, right away.
#[test]
fn effect_runs_once_on_creation() {
  let (runs, count_run) = counter();
  let source = Signal::new(1);

  Effect::new(move || {
    count_run();
    source.get();
  });

  assert_eq!(runs.get(), 1);
}

// A write to something the effect read doesn't run it; it waits for a flush point.
#[test]
fn set_does_not_run_the_effect() {
  let (runs, count_run) = counter();
  let source = Signal::new(1);
  let double = Memo::new(move || source.get() * 2);
  Effect::new(move || {
    count_run();
    double.get();
  });

  source.set(2);

  assert_eq!(runs.get(), 1);
}

// The effect's closure can keep mutable state of its own between runs.
#[test]
fn effect_closure_keeps_its_own_state() {
  let log = Rc::new(RefCell::new(Vec::new()));
  let log_in_effect = Rc::clone(&log);
  let mut runs = 0;

  Effect::new(move || {
    runs += 1;
    log_in_effect.borrow_mut().push(runs);
  });

  assert_eq!(*log.borrow(), [1]);
}
