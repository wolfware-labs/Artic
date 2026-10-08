// Integration tests for `Memo`, using only the public API, as app code would.

use artic_core::reactive::{Memo, Signal};
use pretty_assertions::assert_eq;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

/// Returns a call counter and a closure that bumps it, to count how often a memo computes.
fn counter() -> (Rc<Cell<u32>>, impl Fn() + 'static) {
  let calls = Rc::new(Cell::new(0));
  let calls_in_closure = Rc::clone(&calls);
  (calls, move || calls_in_closure.set(calls_in_closure.get() + 1))
}

// Reading

// `with` borrows the value, so memos of types that aren't `Clone` can be read.
#[test]
fn with_reads_a_value_that_is_not_clone() {
  #[derive(PartialEq)]
  struct Total(i32);
  let source = Signal::new(2);
  let total = Memo::new(move || Total(source.get() * 10));

  assert_eq!(total.with(|total| total.0), 20);
}

// Copies of a memo handle read the same memo.
#[test]
fn copied_handle_reads_the_same_memo() {
  let (calls, count_call) = counter();
  let source = Signal::new(1);
  let double = Memo::new(move || {
    count_call();
    source.get() * 2
  });
  let copy = double;

  assert_eq!(double.get(), 2);
  assert_eq!(copy.get(), 2);
  assert_eq!(calls.get(), 1);
}

// Several readers

// Two memos that read the same memo both see its change, whichever of them is read first.
#[test]
fn both_readers_of_a_memo_see_its_change() {
  let source = Signal::new(1);
  let double = Memo::new(move || source.get() * 2);
  let first = Memo::new(move || double.get() + 1);
  let second = Memo::new(move || double.get() + 2);
  assert_eq!((first.get(), second.get()), (3, 4));

  source.set(5);

  assert_eq!(first.get(), 11);
  assert_eq!(second.get(), 12);
}

// A memo read by two paths computes once per change.
#[test]
fn shared_memo_computes_once_per_change() {
  let (calls, count_call) = counter();
  let source = Signal::new(1);
  let double = Memo::new(move || {
    count_call();
    source.get() * 2
  });
  let left = Memo::new(move || double.get() + 1);
  let right = Memo::new(move || double.get() - 1);
  let sum = Memo::new(move || left.get() + right.get());
  assert_eq!(sum.get(), 4);

  source.set(2);

  assert_eq!(sum.get(), 8);
  assert_eq!(calls.get(), 2);
}

// A memo read only on a branch that is no longer taken is not computed.
#[test]
fn memo_on_an_untaken_branch_is_not_computed() {
  let (calls, count_call) = counter();
  let use_detail = Signal::new(true);
  let source = Signal::new(1);
  let detail = Memo::new(move || {
    count_call();
    source.get() * 2
  });
  let shown = Memo::new(move || if use_detail.get() { detail.get() } else { 0 });
  assert_eq!(shown.get(), 2);

  use_detail.set(false);
  source.set(2);

  assert_eq!(shown.get(), 0);
  assert_eq!(calls.get(), 1);
}

// Panics

// A memo whose closure panics computes again on the next read.
#[test]
fn memo_computes_again_after_its_closure_panics() {
  let fail = Signal::new(true);
  let value = Memo::new(move || {
    assert!(!fail.get(), "compute failed");
    7
  });
  assert!(catch_unwind(AssertUnwindSafe(|| value.get())).is_err());

  fail.set(false);

  assert_eq!(value.get(), 7);
}

// A memo that reads itself panics instead of recursing forever.
#[test]
#[should_panic(expected = "memo read while it is being computed")]
fn memo_reading_itself_panics() {
  let slot: Rc<Cell<Option<Memo<i32>>>> = Rc::new(Cell::new(None));
  let slot_in_closure = Rc::clone(&slot);
  let memo = Memo::new(move || slot_in_closure.get().map_or(0, |memo| memo.get()));
  slot.set(Some(memo));

  memo.get();
}
