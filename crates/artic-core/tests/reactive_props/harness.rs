use crate::graph::{Graph, GraphSpec, NodeKind, Source};
use crate::ops::Op;
use artic_core::reactive::{batch, flush_effects};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// One run of an effect: its node index and the values it read, in input order.
#[derive(Debug, Clone)]
pub struct Run {
  pub effect: usize,
  pub seen: Vec<i64>,
}

/// A built graph, a model of its signal values and live scopes, and a log of every effect run. It
/// applies ops and checks the run log after each one against an oracle that recomputes every value
/// from the signal values, with no caching.
pub struct Harness {
  spec: GraphSpec,
  graph: Graph,
  signal_values: Vec<i64>,
  alive_scopes: Vec<bool>,
  log: Rc<RefCell<Vec<Run>>>,
  checked_runs: usize,
  last_seen: HashMap<usize, Vec<i64>>,
}

impl Harness {
  /// Builds `spec` and checks the creation runs: every effect ran once.
  pub fn new(spec: GraphSpec) -> Harness {
    let log = Rc::new(RefCell::new(Vec::new()));
    let log_in_effects = Rc::clone(&log);
    let graph = Graph::build(&spec, move |effect, seen| {
      log_in_effects.borrow_mut().push(Run { effect, seen })
    });
    let signal_values = spec
      .nodes
      .iter()
      .map(|node| match node.kind {
        NodeKind::Signal { initial } => initial,
        _ => 0,
      })
      .collect();
    let alive_scopes = vec![true; spec.scopes.len()];
    let mut harness = Harness {
      spec,
      graph,
      signal_values,
      alive_scopes,
      log,
      checked_runs: 0,
      last_seen: HashMap::new(),
    };
    let effects = harness
      .spec
      .nodes
      .iter()
      .filter(|node| matches!(node.kind, NodeKind::Effect { .. }));
    assert_eq!(
      harness.log.borrow().len(),
      effects.count(),
      "every effect runs once on creation"
    );
    harness.check_flush();
    harness
  }

  pub fn apply(&mut self, op: &Op) {
    match op {
      Op::Set { signal, value } => {
        self.set(*signal, *value);
        self.check_no_runs("set");
      }
      Op::Batch(sets) => {
        batch(|| {
          for (signal, value) in sets {
            self.set(*signal, *value);
          }
        });
        self.check_flush();
      }
      Op::Flush => {
        flush_effects();
        self.check_flush();
      }
      Op::Dispose { scope } => {
        self.dispose(*scope);
        self.check_no_runs("dispose");
      }
    }
  }

  /// Flushes the remaining changes and disposes the root scope.
  pub fn finish(mut self) {
    flush_effects();
    self.check_flush();
    self.graph.root().dispose();
  }

  /// Sets the `pick`-th live signal, if there is one, in the graph and in the model.
  fn set(&mut self, pick: usize, value: i64) {
    let signals: Vec<usize> = (0..self.spec.nodes.len())
      .filter(|&index| matches!(self.spec.nodes[index].kind, NodeKind::Signal { .. }) && self.is_alive(index))
      .collect();
    if signals.is_empty() {
      return;
    }
    let index = signals[pick % signals.len()];
    let Some(Source::Signal(signal)) = self.graph.sources[index] else {
      unreachable!("signal nodes build signals");
    };
    signal.set(value);
    self.signal_values[index] = value;
  }

  /// Disposes the `pick`-th live scope other than the root, if there is one, and marks its subtree
  /// as dead.
  fn dispose(&mut self, pick: usize) {
    let scopes: Vec<usize> = (1..self.spec.scopes.len())
      .filter(|&scope| self.alive_scopes[scope])
      .collect();
    if scopes.is_empty() {
      return;
    }
    let disposed = scopes[pick % scopes.len()];
    self.graph.scopes[disposed].dispose();
    for scope in 0..self.spec.scopes.len() {
      if self.spec.is_ancestor_or_self(disposed, scope) {
        self.alive_scopes[scope] = false;
      }
    }
  }

  /// The oracle: the value of a signal or memo node, recomputed from the signal values.
  fn expected_value(&self, node: usize) -> i64 {
    match &self.spec.nodes[node].kind {
      NodeKind::Signal { .. } => self.signal_values[node],
      NodeKind::Memo { inputs, modulus } => inputs.iter().map(|input| self.expected_value(*input)).sum::<i64>() % modulus,
      NodeKind::Effect { .. } => unreachable!("effects are never read"),
    }
  }

  /// The values the oracle expects an effect to read.
  fn expected_seen(&self, effect: usize) -> Vec<i64> {
    let NodeKind::Effect { inputs } = &self.spec.nodes[effect].kind else {
      unreachable!("only effects run");
    };
    inputs.iter().map(|input| self.expected_value(*input)).collect()
  }

  fn is_alive(&self, node: usize) -> bool {
    self.alive_scopes[self.spec.nodes[node].scope]
  }

  /// Checks that an op that isn't a flush point ran no effect.
  fn check_no_runs(&self, op: &str) {
    assert_eq!(
      self.log.borrow().len(),
      self.checked_runs,
      "{op} ran an effect outside a flush point"
    );
  }

  /// Checks the runs since the last flush point: each effect ran at most once, only if alive, and
  /// read the values the oracle expects. Then checks that every live effect's latest run read the
  /// current values, so none missed an update.
  fn check_flush(&mut self) {
    let runs = self.log.borrow()[self.checked_runs..].to_vec();
    let mut ran = HashSet::new();
    for run in runs {
      assert!(
        self.is_alive(run.effect),
        "effect {} ran after its scope was disposed",
        run.effect
      );
      assert!(ran.insert(run.effect), "effect {} ran twice in one flush", run.effect);
      assert_eq!(
        run.seen,
        self.expected_seen(run.effect),
        "effect {} saw an inconsistent snapshot",
        run.effect
      );
      self.last_seen.insert(run.effect, run.seen);
    }
    self.checked_runs = self.log.borrow().len();
    for (&effect, seen) in &self.last_seen {
      if self.is_alive(effect) {
        assert_eq!(seen, &self.expected_seen(effect), "effect {effect} missed an update");
      }
    }
  }
}
