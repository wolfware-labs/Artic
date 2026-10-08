// Integration tests for `Effect`, using only the public API, as app code would.

use artic_core::reactive::{Effect, Memo, Signal, flush_effects};
use pretty_assertions::assert_eq;
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
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

// Flushing

// A flush reruns an effect whose dependency changed.
#[test]
fn flush_reruns_effect_after_a_change() {
  let source = Signal::new(1);
  let seen = Rc::new(Cell::new(0));
  let seen_in_effect = Rc::clone(&seen);
  Effect::new(move || seen_in_effect.set(source.get()));

  source.set(2);
  flush_effects();

  assert_eq!(seen.get(), 2);
}

// A flush with no writes since the last one runs nothing.
#[test]
fn flush_without_changes_runs_nothing() {
  let (runs, count_run) = counter();
  let source = Signal::new(1);
  Effect::new(move || {
    count_run();
    source.get();
  });

  flush_effects();
  flush_effects();

  assert_eq!(runs.get(), 1);
}

// An effect behind a memo whose value didn't change doesn't run.
#[test]
fn unchanged_memo_does_not_rerun_effect() {
  let (runs, count_run) = counter();
  let source = Signal::new(2);
  let is_even = Memo::new(move || source.get() % 2 == 0);
  Effect::new(move || {
    count_run();
    is_even.get();
  });

  source.set(4);
  flush_effects();

  assert_eq!(runs.get(), 1);
}

// An effect that stopped reading a signal doesn't run when that signal changes.
#[test]
fn effect_drops_stale_conditional_dependency() {
  let (runs, count_run) = counter();
  let use_detail = Signal::new(true);
  let detail = Signal::new(1);
  Effect::new(move || {
    count_run();
    if use_detail.get() {
      detail.get();
    }
  });
  use_detail.set(false);
  flush_effects();

  detail.set(2);
  flush_effects();

  assert_eq!(runs.get(), 2);
}

// Re-entrancy

// When an effect writes the input of an effect created before it, the flush runs another pass, so
// both have settled when it returns.
#[test]
fn write_during_flush_settles_in_the_same_flush() {
  let input = Signal::new(1);
  let total = Signal::new(0);
  let seen_total = Rc::new(Cell::new(0));
  let seen_in_effect = Rc::clone(&seen_total);
  Effect::new(move || seen_in_effect.set(total.get()));
  Effect::new(move || total.set(input.get() * 10));
  flush_effects();

  input.set(2);
  flush_effects();

  assert_eq!(seen_total.get(), 20);
}

// Two effects that keep writing each other's inputs make the flush panic instead of looping forever.
#[test]
#[should_panic(expected = "effects did not settle after 100 passes")]
fn effects_that_never_settle_panic() {
  let ping = Signal::new(0);
  let pong = Signal::new(0);
  Effect::new(move || pong.set(ping.get() + 1));
  Effect::new(move || ping.set(pong.get() + 1));

  flush_effects();
}

// Calling `flush_effects` from inside an effect does nothing; the running flush handles the change.
#[test]
fn flush_inside_an_effect_does_nothing() {
  let (runs, count_run) = counter();
  let source = Signal::new(1);
  Effect::new(move || {
    count_run();
    source.get();
    flush_effects();
  });

  source.set(2);
  flush_effects();

  assert_eq!(runs.get(), 2);
}

// An effect created during a flush runs once when created and joins later flushes.
#[test]
fn effect_created_during_a_flush_runs_and_joins_later_flushes() {
  let (inner_runs, count_inner_run) = counter();
  let count_inner_run = Rc::new(count_inner_run);
  let create = Signal::new(false);
  let source = Signal::new(1);
  Effect::new(move || {
    if create.get() {
      let count_inner_run = Rc::clone(&count_inner_run);
      Effect::new(move || {
        count_inner_run();
        source.get();
      });
    }
  });

  create.set(true);
  flush_effects();
  assert_eq!(inner_runs.get(), 1);

  source.set(2);
  flush_effects();
  assert_eq!(inner_runs.get(), 2);
}

// Panics

// An effect that panics during a flush runs again on the next flush, even with no write in between.
#[test]
fn panicking_effect_runs_again_on_next_flush() {
  let fail = Rc::new(Cell::new(false));
  let fail_in_effect = Rc::clone(&fail);
  let source = Signal::new(1);
  let seen = Rc::new(Cell::new(0));
  let seen_in_effect = Rc::clone(&seen);
  Effect::new(move || {
    let value = source.get();
    assert!(!fail_in_effect.get(), "effect failed");
    seen_in_effect.set(value);
  });
  fail.set(true);
  source.set(2);
  assert!(catch_unwind(AssertUnwindSafe(flush_effects)).is_err());

  fail.set(false);
  flush_effects();

  assert_eq!(seen.get(), 2);
}
