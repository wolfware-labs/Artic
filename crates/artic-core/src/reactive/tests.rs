use crate::reactive::observable::ObservableId;
use crate::reactive::runtime::{ObserverId, Runtime};
use crate::reactive::{Memo, Signal, run_tracked};
use pretty_assertions::assert_eq;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

/// Creates an observer and runs `action` as that observer, so every signal read inside subscribes it.
fn observe(action: impl FnOnce()) -> ObserverId {
  let observer = Runtime::create_observer();
  run_tracked(observer, action);
  observer
}

// Reading a signal outside any observer records no dependency.
#[test]
fn get_outside_observer_records_nothing() {
  let count = Signal::new(0);
  let observer = Runtime::create_observer();

  count.get();
  count.set(1);

  assert!(!Runtime::is_observer_dirty(observer));
}

// Reading a signal inside `run_tracked` records the running observer as a subscriber.
#[test]
fn get_inside_observer_records_dependency() {
  let count = Signal::new(0);
  let observer = observe(|| {
    count.get();
  });

  count.set(1);

  assert!(Runtime::is_observer_dirty(observer));
}

// `set` marks every observer that read the signal as dirty.
#[test]
fn set_marks_dependents_dirty() {
  let count = Signal::new(0);
  let first = observe(|| {
    count.get();
  });
  let second = observe(|| {
    count.get();
  });
  assert!(!Runtime::is_observer_dirty(first));
  assert!(!Runtime::is_observer_dirty(second));

  count.set(1);

  assert!(Runtime::is_observer_dirty(first));
  assert!(Runtime::is_observer_dirty(second));
}

// `set_if_changed` with a value equal to the current one does not mark dependents dirty (`PartialEq`
// types). Plain `set` always propagates: see `set_with_equal_value_still_marks_dependents_dirty`.
#[test]
fn equal_set_does_not_propagate() {
  let count = Signal::new(1);
  let observer = observe(|| {
    count.get();
  });

  count.set_if_changed(1);

  assert!(!Runtime::is_observer_dirty(observer));
}

// `set` marks dependents dirty even when the new value equals the current one.
#[test]
fn set_with_equal_value_still_marks_dependents_dirty() {
  let count = Signal::new(1);
  let observer = observe(|| {
    count.get();
  });

  count.set(1);

  assert!(Runtime::is_observer_dirty(observer));
}

// `set_if_changed` with a different value marks dependents dirty, the same as `set`.
#[test]
fn set_if_changed_with_new_value_marks_dependents_dirty() {
  let count = Signal::new(1);
  let observer = observe(|| {
    count.get();
  });

  count.set_if_changed(2);

  assert!(Runtime::is_observer_dirty(observer));
}

// The comparison inside `set_if_changed` does not subscribe the running observer: it is a write.
#[test]
fn set_if_changed_inside_observer_records_nothing() {
  let count = Signal::new(0);
  let observer = observe(|| count.set_if_changed(1));

  count.set(5);

  assert!(!Runtime::is_observer_dirty(observer));
}

// Tracking and dirty marking

// Reading through `with` records a dependency, the same as `get`.
#[test]
fn with_inside_observer_records_dependency() {
  let name = Signal::new(String::from("Ada"));
  let observer = observe(|| {
    name.with(|name| name.len());
  });

  name.set(String::from("Grace"));

  assert!(Runtime::is_observer_dirty(observer));
}

// `update` marks dependents dirty, the same as `set`.
#[test]
fn update_marks_dependents_dirty() {
  let items = Signal::new(vec![1, 2]);
  let observer = observe(|| {
    items.get();
  });

  items.update(|items| items.push(3));

  assert!(Runtime::is_observer_dirty(observer));
}

// An observer that read a different signal is not marked dirty.
#[test]
fn set_leaves_unrelated_observers_clean() {
  let read = Signal::new(0);
  let written = Signal::new(0);
  let observer = observe(|| {
    read.get();
  });

  written.set(1);

  assert!(!Runtime::is_observer_dirty(observer));
}

// Running an observer again clears its dirty flag, so only writes after the run mark it dirty.
#[test]
fn run_tracked_marks_observer_clean() {
  let count = Signal::new(0);
  let observer = observe(|| {
    count.get();
  });
  count.set(1);

  run_tracked(observer, || {
    count.get();
  });

  assert!(!Runtime::is_observer_dirty(observer));
}

// `run_tracked` returns every signal the action read, once each and in read order.
#[test]
fn run_tracked_returns_each_signal_read_once() {
  let first = Signal::new(0);
  let second = Signal::new(0);
  let observer = Runtime::create_observer();

  let observables = run_tracked(observer, || {
    first.get();
    second.with(|_| ());
    first.get();
  });

  assert_eq!(
    observables,
    [ObservableId::Signal(first.id()), ObservableId::Signal(second.id())]
  );
}

// Unsubscribing an observer from what it read stops writes from marking it dirty.
#[test]
fn unsubscribed_observer_is_not_marked_dirty() {
  let count = Signal::new(0);
  let observer = Runtime::create_observer();
  let observables = run_tracked(observer, || {
    count.get();
  });

  for observable in observables {
    observable.unsubscribe(observer);
  }
  count.set(1);

  assert!(!Runtime::is_observer_dirty(observer));
}

// Panic safety

// If the action passed to `run_tracked` panics, the observer is unsubscribed from what it read before
// the panic, because nothing returns that list to the caller.
#[test]
fn run_tracked_unsubscribes_reads_when_action_panics() {
  let count = Signal::new(0);
  let observer = Runtime::create_observer();

  let result = catch_unwind(AssertUnwindSafe(|| {
    run_tracked(observer, || {
      count.get();
      panic!("action failed");
    })
  }));
  count.set(1);

  assert!(result.is_err());
  assert!(!Runtime::is_observer_dirty(observer));
}

// If the action passed to `run_tracked` panics, the observer is popped and the panic still reaches the caller.
#[test]
fn run_tracked_pops_observer_when_action_panics() {
  let count = Signal::new(0);
  let observer = Runtime::create_observer();

  let result = catch_unwind(AssertUnwindSafe(|| run_tracked(observer, || panic!("action failed"))));

  let payload = result.expect_err("the panic should reach the caller of run_tracked");
  assert_eq!(payload.downcast_ref::<&str>(), Some(&"action failed"));

  // If the observer were still on the stack, this read would subscribe it.
  count.get();
  count.set(1);
  assert!(!Runtime::is_observer_dirty(observer));
}

// If an `update` closure panics, dependents are still marked dirty before the panic continues.
#[test]
fn panicking_update_still_marks_dependents_dirty() {
  let count = Signal::new(0);
  let observer = observe(|| {
    count.get();
  });

  let result = catch_unwind(AssertUnwindSafe(|| count.update(|_| panic!("update failed"))));

  assert!(result.is_err());
  assert!(Runtime::is_observer_dirty(observer));
}

// Nesting

// When observers nest, a read subscribes only the innermost running observer.
#[test]
fn nested_run_tracked_subscribes_only_innermost_observer() {
  let count = Signal::new(0);
  let outer = Runtime::create_observer();
  let inner = Runtime::create_observer();

  run_tracked(outer, || {
    run_tracked(inner, || {
      count.get();
    });
  });
  count.set(1);

  assert!(Runtime::is_observer_dirty(inner));
  assert!(!Runtime::is_observer_dirty(outer));
}

// Once a nested observer finishes, reads subscribe the outer observer again.
#[test]
fn outer_observer_is_current_again_after_inner_finishes() {
  let inner_signal = Signal::new(0);
  let outer_signal = Signal::new(0);
  let outer = Runtime::create_observer();
  let inner = Runtime::create_observer();

  run_tracked(outer, || {
    run_tracked(inner, || {
      inner_signal.get();
    });
    outer_signal.get();
  });
  outer_signal.set(1);

  assert!(Runtime::is_observer_dirty(outer));
  assert!(!Runtime::is_observer_dirty(inner));
}

// If a nested observer panics and the outer one catches it, the outer observer is current again.
#[test]
fn inner_panic_restores_outer_observer() {
  let count = Signal::new(0);
  let outer = Runtime::create_observer();
  let inner = Runtime::create_observer();

  run_tracked(outer, || {
    let result = catch_unwind(AssertUnwindSafe(|| run_tracked(inner, || panic!("inner failed"))));
    assert!(result.is_err());
    count.get();
  });
  count.set(1);

  assert!(Runtime::is_observer_dirty(outer));
  assert!(!Runtime::is_observer_dirty(inner));
}

// More tracking rules

// An observer that read two signals is marked dirty by a write to either of them.
#[test]
fn observer_of_two_signals_is_dirtied_by_either() {
  for write_first in [true, false] {
    let first = Signal::new(0);
    let second = Signal::new(0);
    let observer = observe(|| {
      first.get();
      second.get();
    });

    if write_first {
      first.set(1)
    } else {
      second.set(1)
    }

    assert!(Runtime::is_observer_dirty(observer), "write_first = {write_first}");
  }
}

// Creating a signal while an observer runs does not subscribe the observer to it.
#[test]
fn creating_signal_inside_observer_records_nothing() {
  let mut created = None;
  let observer = observe(|| created = Some(Signal::new(0)));

  created.expect("the signal should have been created").set(1);

  assert!(!Runtime::is_observer_dirty(observer));
}

// Writing a signal while an observer runs does not subscribe the observer: only reads do.
#[test]
fn update_inside_observer_records_nothing() {
  let count = Signal::new(0);
  let observer = observe(|| count.update(|count| *count += 1));

  count.set(5);

  assert!(!Runtime::is_observer_dirty(observer));
}

// Memo

/// Returns a call counter and a closure that bumps it, to count how often a memo computes.
fn counter() -> (Rc<Cell<u32>>, impl Fn() + 'static) {
  let calls = Rc::new(Cell::new(0));
  let calls_in_closure = Rc::clone(&calls);
  (calls, move || calls_in_closure.set(calls_in_closure.get() + 1))
}

// Creating a memo runs nothing; the first read computes it.
#[test]
fn memo_not_computed_until_read() {
  let (calls, count_call) = counter();
  let source = Signal::new(2);
  let double = Memo::new(move || {
    count_call();
    source.get() * 2
  });
  assert_eq!(calls.get(), 0);

  assert_eq!(double.get(), 4);
  assert_eq!(calls.get(), 1);
}

// After a dependency changes, the memo computes once on the next read and remembers the result.
#[test]
fn memo_recomputes_once_after_dependency_change() {
  let (calls, count_call) = counter();
  let source = Signal::new(2);
  let double = Memo::new(move || {
    count_call();
    source.get() * 2
  });
  double.get();

  source.set(3);
  source.set(4);
  assert_eq!(calls.get(), 1, "a change alone must not compute");

  assert_eq!(double.get(), 8);
  assert_eq!(double.get(), 8);
  assert_eq!(calls.get(), 2);
}

// A dependency read only on a branch that is no longer taken stops causing computes.
#[test]
fn memo_drops_stale_conditional_dependency() {
  let (calls, count_call) = counter();
  let use_detail = Signal::new(true);
  let detail = Signal::new(1);
  let shown = Memo::new(move || {
    count_call();
    if use_detail.get() { detail.get() } else { 0 }
  });
  assert_eq!(shown.get(), 1);
  use_detail.set(false);
  assert_eq!(shown.get(), 0);

  detail.set(2);

  assert_eq!(shown.get(), 0);
  assert_eq!(calls.get(), 2, "changing a dropped dependency must not compute");
}

// A change to a signal reaches the end of a chain of memos.
#[test]
fn memo_chain_propagates() {
  let source = Signal::new(1);
  let double = Memo::new(move || source.get() * 2);
  let label = Memo::new(move || format!("value: {}", double.get()));
  assert_eq!(label.get(), "value: 2");

  source.set(5);

  assert_eq!(label.get(), "value: 10");
}

// A memo that computes a value equal to its last one leaves the memos that read it up to date.
#[test]
fn equal_recompute_does_not_dirty_dependents() {
  let (calls, count_call) = counter();
  let source = Signal::new(2);
  let is_even = Memo::new(move || source.get() % 2 == 0);
  let label = Memo::new(move || {
    count_call();
    if is_even.get() { "even" } else { "odd" }
  });
  assert_eq!(label.get(), "even");

  source.set(4);

  assert_eq!(label.get(), "even");
  assert_eq!(calls.get(), 1, "label must not compute when is_even is unchanged");
}

// A memo read inside a tracked run is returned once, as a memo and not as its inner value signal.
#[test]
fn run_tracked_records_a_memo_read_as_the_memo() {
  let source = Signal::new(1);
  let double = Memo::new(move || source.get() * 2);
  let observer = Runtime::create_observer();

  let observables = run_tracked(observer, || {
    double.get();
    double.with(|_| ());
  });

  assert_eq!(observables.len(), 1);
  assert!(matches!(observables[0], ObservableId::Memo(_)));
}
