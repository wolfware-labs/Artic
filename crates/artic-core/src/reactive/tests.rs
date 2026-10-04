// Reading a signal outside any observer records no dependency.
#[test]
fn get_outside_observer_records_nothing() {
  todo!()
}

// Reading a signal inside `run_tracked` records the running observer as a subscriber.
#[test]
fn get_inside_observer_records_dependency() {
  todo!()
}

// `set` marks every observer that read the signal as dirty.
#[test]
fn set_marks_dependents_dirty() {
  todo!()
}

// `set` with a value equal to the current one does not mark dependents dirty (`PartialEq` types).
#[test]
fn equal_set_does_not_propagate() {
  todo!()
}

// Tracking and dirty marking

// Reading through `with` records a dependency, the same as `get`.
#[test]
fn with_inside_observer_records_dependency() {
  todo!()
}

// Reading the same signal twice in one observer subscribes the observer only once.
#[test]
fn repeated_reads_subscribe_once() {
  todo!()
}

// `update` marks dependents dirty, the same as `set`.
#[test]
fn update_marks_dependents_dirty() {
  todo!()
}

// An observer that read a different signal is not marked dirty.
#[test]
fn set_leaves_unrelated_observers_clean() {
  todo!()
}

// Panic safety

// If the action passed to `run_tracked` panics, the observer is popped and the panic still reaches the caller.
#[test]
fn run_tracked_pops_observer_when_action_panics() {
  todo!()
}

// If an `update` closure panics, dependents are still marked dirty before the panic continues.
#[test]
fn panicking_update_still_marks_dependents_dirty() {
  todo!()
}
