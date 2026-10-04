// Integration tests for `Signal`, using only the public API, as app code would.

// Reading and writing

// `get` returns the value passed to `Signal::new`.
#[test]
fn get_returns_initial_value() {
  todo!()
}

// `get` after `set` returns the new value.
#[test]
fn get_after_set_returns_new_value() {
  todo!()
}

// `get` after `update` returns the updated value.
#[test]
fn get_after_update_returns_updated_value() {
  todo!()
}

// A non-`Copy` value (`String`) survives `update`, `set` and `get` unchanged.
#[test]
fn string_round_trip_through_update_set_and_get() {
  todo!()
}

// `with` returns whatever its closure returns.
#[test]
fn with_returns_closure_result() {
  todo!()
}

// A `Signal<()>` works through `set`, `update`, `with` and `get`.
#[test]
fn unit_signal_works() {
  todo!()
}

// Copies of a signal handle read and write the same value.
#[test]
fn copied_handle_shares_value() {
  todo!()
}

// Nested access

// `with` on one signal can read another signal with `with` and `get`.
#[test]
fn nested_reads_work() {
  todo!()
}

// `with` on one signal can `set` and `update` another signal.
#[test]
fn writing_another_signal_inside_with_works() {
  todo!()
}

// An `update` closure can read another signal.
#[test]
fn update_can_read_another_signal() {
  todo!()
}

// Reading a signal inside its own `update` closure panics with a clear message.
#[test]
#[should_panic(expected = "signal read while it is being updated")]
fn read_inside_own_update_panics() {
  todo!()
}

// Updating a signal inside its own `update` closure panics with a clear message.
#[test]
#[should_panic(expected = "signal updated while it is already being updated")]
fn update_inside_own_update_panics() {
  todo!()
}

// Panic safety

// After an `update` closure panics, the panic reaches the caller and the signal is still usable.
#[test]
fn panicking_update_keeps_signal_usable() {
  todo!()
}
