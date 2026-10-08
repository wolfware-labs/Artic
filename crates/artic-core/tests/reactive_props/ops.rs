use proptest::collection::vec;
use proptest::prelude::*;

/// One edit applied to a built graph. Indexes are raw picks: the harness maps them onto the signals
/// and scopes still alive, and skips the op when there are none.
#[derive(Debug, Clone)]
pub enum Op {
  Set { signal: usize, value: i64 },
  Batch(Vec<(usize, i64)>),
  Flush,
  Dispose { scope: usize },
}

/// Generates up to 24 ops, with batches of 1 to 4 sets.
pub fn ops() -> impl Strategy<Value = Vec<Op>> {
  let set = (any::<usize>(), 0..10i64);
  let op = prop_oneof![
    set.clone().prop_map(|(signal, value)| Op::Set { signal, value }),
    vec(set, 1..=4).prop_map(Op::Batch),
    Just(Op::Flush),
    any::<usize>().prop_map(|scope| Op::Dispose { scope }),
  ];
  vec(op, 0..=24)
}
