// Property-based tests for the reactive core: random graphs and random edits, checked against
// rules that must always hold. Run more cases with `PROPTEST_CASES=10000`.

use artic_core::reactive::Signal;
use proptest::prelude::*;

proptest! {
  // A value written to a signal is the value read back.
  #[test]
  fn signal_reads_back_the_value_set(initial in any::<i64>(), value in any::<i64>()) {
    let signal = Signal::new(initial);
    signal.set(value);
    prop_assert_eq!(signal.get(), value);
  }
}
