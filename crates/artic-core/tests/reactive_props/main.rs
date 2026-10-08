// Property-based tests for the reactive core: random graphs and random edits, checked against
// rules that must always hold. Run more cases with `PROPTEST_CASES=10000`.

mod graph;

use artic_core::reactive::{assert_no_reactive_leaks, live_counts};
use graph::{Graph, graph};
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
}
