// Property-based tests for the reactive core: random graphs and random edits, checked against
// rules that must always hold. Run more cases with `PROPTEST_CASES=10000`.

mod graph;
mod harness;
mod ops;

use artic_core::reactive::{assert_no_reactive_leaks, live_counts};
use graph::{Graph, graph};
use harness::Harness;
use ops::ops;
use proptest::prelude::*;

proptest! {
  // Disposing the root scope frees every signal, memo, effect and scope of any graph.
  #[test]
  fn disposing_the_root_frees_every_node(spec in graph()) {
    let baseline = live_counts();
    let graph = Graph::build(&spec, |_, _| {});

    graph.root().dispose();

    assert_no_reactive_leaks(baseline);
  }

  // Under any sequence of sets, batches, flushes and disposals, effects run only at flush points,
  // at most once per flush, and never after their scope is disposed; disposing the root at the end
  // frees every node.
  #[test]
  fn random_edits_run_effects_once_per_flush(spec in graph(), ops in ops()) {
    let baseline = live_counts();
    let mut harness = Harness::new(spec);

    for op in &ops {
      harness.apply(op);
    }
    harness.finish();

    assert_no_reactive_leaks(baseline);
  }
}
