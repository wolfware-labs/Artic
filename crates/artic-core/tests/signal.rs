// Integration tests for `Signal`, using only the public API, as app code would.

use artic_core::reactive::Signal;
use pretty_assertions::assert_eq;
use std::panic::{AssertUnwindSafe, catch_unwind};

// Reading and writing

// `get` returns the value passed to `Signal::new`.
#[test]
fn get_returns_initial_value() {
  let count = Signal::new(42);
  assert_eq!(count.get(), 42);
}

// `get` after `set` returns the new value.
#[test]
fn get_after_set_returns_new_value() {
  let count = Signal::new(1);
  count.set(2);
  assert_eq!(count.get(), 2);
}

// `get` after `update` returns the updated value.
#[test]
fn get_after_update_returns_updated_value() {
  let count = Signal::new(1);
  count.update(|count| *count += 10);
  assert_eq!(count.get(), 11);
}

// A non-`Copy` value (`String`) survives `update`, `set` and `get` unchanged.
#[test]
fn string_round_trip_through_update_set_and_get() {
  let name = Signal::new(String::from("Ada"));

  name.update(|name| name.push_str(" Lovelace"));
  assert_eq!(name.get(), "Ada Lovelace");

  name.set(String::from("Grace Hopper"));
  assert_eq!(name.get(), "Grace Hopper");
}

// `with` returns whatever its closure returns.
#[test]
fn with_returns_closure_result() {
  let items = Signal::new(vec![1, 2, 3]);
  let total: i32 = items.with(|items| items.iter().sum());
  assert_eq!(total, 6);
}

// A `Signal<()>` works through `set`, `update`, `with` and `get`.
#[test]
fn unit_signal_works() {
  let trigger = Signal::new(());

  trigger.set(());
  trigger.update(|_| {});

  assert_eq!(trigger.with(|value| *value), ());
  assert_eq!(trigger.get(), ());
}

// Copies of a signal handle read and write the same value.
#[test]
fn copied_handle_shares_value() {
  let original = Signal::new(1);
  let copy = original;

  copy.set(2);
  assert_eq!(original.get(), 2);

  original.update(|value| *value += 1);
  assert_eq!(copy.get(), 3);
}

// Nested access

// `with` on one signal can read another signal with `with` and `get`.
#[test]
fn nested_reads_work() {
  let first = Signal::new(1);
  let second = Signal::new(10);

  let sum = first.with(|first| first + second.get() + second.with(|second| *second));

  assert_eq!(sum, 21);
}

// `with` on one signal can `set` and `update` another signal.
#[test]
fn writing_another_signal_inside_with_works() {
  let source = Signal::new(5);
  let target = Signal::new(0);

  source.with(|source| {
    target.set(*source);
    target.update(|target| *target *= 2);
  });

  assert_eq!(target.get(), 10);
}

// An `update` closure can read another signal.
#[test]
fn update_can_read_another_signal() {
  let step = Signal::new(3);
  let count = Signal::new(1);

  count.update(|count| *count += step.get());

  assert_eq!(count.get(), 4);
}

// Reading a signal inside its own `update` closure panics with a clear message.
#[test]
#[should_panic(expected = "signal read while it is being updated")]
fn read_inside_own_update_panics() {
  let count = Signal::new(1);
  count.update(|_| {
    count.get();
  });
}

// Updating a signal inside its own `update` closure panics with a clear message.
#[test]
#[should_panic(expected = "signal updated while it is already being updated")]
fn update_inside_own_update_panics() {
  let count = Signal::new(1);
  count.update(|_| count.update(|_| {}));
}

// Panic safety

// After an `update` closure panics, the panic reaches the caller and the signal is still usable.
#[test]
fn panicking_update_keeps_signal_usable() {
  let count = Signal::new(1);

  let result = catch_unwind(AssertUnwindSafe(|| {
    count.update(|count| {
      *count = 5;
      panic!("update failed");
    })
  }));

  let payload = result.expect_err("the panic should reach the caller of update");
  assert_eq!(payload.downcast_ref::<&str>(), Some(&"update failed"));

  // The change made before the panic is kept, and the signal can still be read and written.
  assert_eq!(count.get(), 5);
  count.set(6);
  assert_eq!(count.get(), 6);
}

// Independence

// Two signals hold separate values, even when they have the same type.
#[test]
fn signals_are_independent() {
  let first = Signal::new(1);
  let second = Signal::new(2);

  first.set(10);

  assert_eq!(first.get(), 10);
  assert_eq!(second.get(), 2);
}

// A value that isn't `Clone` works through `with`, `update` and `set`.
#[test]
fn non_clone_value_works_through_with_update_and_set() {
  #[derive(Debug, PartialEq)]
  struct Token(u32);

  let token = Signal::new(Token(1));

  token.update(|token| token.0 += 1);
  assert_eq!(token.with(|token| token.0), 2);

  token.set(Token(7));
  assert!(token.with(|token| *token == Token(7)));
}

// Creating signals during access

// A signal can be created inside another signal's `with` closure.
#[test]
fn creating_signal_inside_with_works() {
  let source = Signal::new(3);

  let created = source.with(|source| Signal::new(*source * 2));

  assert_eq!(created.get(), 6);
  assert_eq!(source.get(), 3);
}

// A signal can be created inside another signal's `update` closure.
#[test]
fn creating_signal_inside_update_works() {
  let count = Signal::new(1);
  let mut created = None;

  count.update(|count| {
    *count += 1;
    created = Some(Signal::new(*count));
  });

  assert_eq!(count.get(), 2);
  assert_eq!(created.expect("the signal should have been created").get(), 2);
}

// Creating many signals inside `with` grows the runtime's storage without invalidating the value
// being read.
#[test]
fn creating_many_signals_inside_with_keeps_value_valid() {
  let name = Signal::new(String::from("Ada"));

  let length = name.with(|name| {
    let created: Vec<Signal<usize>> = (0..1_000).map(Signal::new).collect();
    assert_eq!(created[999].get(), 999);
    name.len()
  });

  assert_eq!(length, 3);
  assert_eq!(name.get(), "Ada");
}

// More nested access

// A signal can be read again inside its own `with` closure.
#[test]
fn reading_same_signal_inside_its_own_with_works() {
  let count = Signal::new(4);

  let sum = count.with(|outer| outer + count.get() + count.with(|inner| *inner));

  assert_eq!(sum, 12);
}

// Writing a signal inside its own `with` closure panics.
// TODO: add `expected = "..."` once the clear message for this case exists.
#[test]
#[should_panic]
fn write_inside_own_with_panics() {
  let count = Signal::new(1);
  count.with(|_| count.set(2));
}

// More panic safety

// After a `with` closure panics, the panic reaches the caller and the signal is still usable.
#[test]
fn panicking_with_keeps_signal_usable() {
  let count = Signal::new(1);

  let result = catch_unwind(AssertUnwindSafe(|| count.with(|_| panic!("with failed"))));

  assert!(result.is_err());
  count.set(2);
  assert_eq!(count.get(), 2);
}

// Skipping equal writes

// `set_if_changed` stores a different value and keeps the current one when they are equal.
#[test]
fn set_if_changed_stores_only_different_values() {
  let name = Signal::new(String::from("Ada"));

  name.set_if_changed(String::from("Ada"));
  assert_eq!(name.get(), "Ada");

  name.set_if_changed(String::from("Grace"));
  assert_eq!(name.get(), "Grace");
}

// `set_if_changed` inside the signal's own `update` closure panics, reporting the read it does first.
#[test]
#[should_panic(expected = "signal read while it is being updated")]
fn set_if_changed_inside_own_update_panics() {
  let count = Signal::new(1);
  count.update(|_| count.set_if_changed(2));
}
