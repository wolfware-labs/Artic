use artic_core::reactive::{Memo, Scope, Signal, effect};
use proptest::collection::vec;
use proptest::prelude::*;

/// A random reactive graph as plain data: a scope tree and nodes that read earlier nodes.
#[derive(Debug, Clone)]
pub struct GraphSpec {
  /// The parent of each scope. Scope 0 is the root and has none.
  pub scopes: Vec<Option<usize>>,
  pub nodes: Vec<NodeSpec>,
}

/// One node of a [`GraphSpec`] and the scope it is created in.
#[derive(Debug, Clone)]
pub struct NodeSpec {
  pub scope: usize,
  pub kind: NodeKind,
}

/// What a node is. Inputs are indexes of earlier signal or memo nodes.
#[derive(Debug, Clone)]
pub enum NodeKind {
  Signal { initial: i64 },
  Memo { inputs: Vec<usize>, modulus: i64 },
  Effect { inputs: Vec<usize> },
}

/// Raw random numbers for one node, before [`GraphSpec::normalize`] makes them valid.
#[derive(Debug, Clone)]
struct RawNode {
  kind: u8,
  scope: usize,
  initial: i64,
  inputs: Vec<usize>,
  modulus: i64,
}

/// Generates graphs of 1 to 4 scopes and 1 to 16 nodes.
pub fn graph() -> impl Strategy<Value = GraphSpec> {
  let raw_node = (0..3u8, any::<usize>(), 0..10i64, vec(any::<usize>(), 1..=3), 2..=10i64).prop_map(
    |(kind, scope, initial, inputs, modulus)| RawNode {
      kind,
      scope,
      initial,
      inputs,
      modulus,
    },
  );
  (vec(any::<usize>(), 0..4), vec(raw_node, 1..=16))
    .prop_map(|(scope_parents, nodes)| GraphSpec::normalize(&scope_parents, &nodes))
}

impl GraphSpec {
  /// Turns raw numbers into a valid graph: every scope's parent comes before it, and every node
  /// reads only earlier signals and memos in its own scope or an ancestor scope. A memo or effect
  /// with nothing to read becomes a signal.
  fn normalize(scope_parents: &[usize], raw_nodes: &[RawNode]) -> GraphSpec {
    let mut scopes = vec![None];
    for (index, parent) in scope_parents.iter().enumerate() {
      scopes.push(Some(parent % (index + 1)));
    }
    let mut graph = GraphSpec {
      scopes,
      nodes: Vec::new(),
    };
    for raw in raw_nodes {
      let scope = raw.scope % graph.scopes.len();
      let readable: Vec<usize> = (0..graph.nodes.len()).filter(|&index| graph.can_read(scope, index)).collect();
      let inputs: Vec<usize> = raw
        .inputs
        .iter()
        .filter(|_| !readable.is_empty())
        .map(|pick| readable[pick % readable.len()])
        .collect();
      let kind = match raw.kind {
        1 if !inputs.is_empty() => NodeKind::Memo {
          inputs,
          modulus: raw.modulus,
        },
        2 if !inputs.is_empty() => NodeKind::Effect { inputs },
        _ => NodeKind::Signal { initial: raw.initial },
      };
      graph.nodes.push(NodeSpec { scope, kind });
    }
    graph
  }

  /// Whether a node in `scope` may read node `index`: it is a signal or memo, and its scope is
  /// `scope` or an ancestor of it, so it lives at least as long as the reader.
  fn can_read(&self, scope: usize, index: usize) -> bool {
    let node = &self.nodes[index];
    !matches!(node.kind, NodeKind::Effect { .. }) && self.is_ancestor_or_self(node.scope, scope)
  }

  /// Whether `ancestor` is `scope` or one of its ancestors.
  pub fn is_ancestor_or_self(&self, ancestor: usize, scope: usize) -> bool {
    let mut current = Some(scope);
    while let Some(index) = current {
      if index == ancestor {
        return true;
      }
      current = self.scopes[index];
    }
    false
  }
}

/// A signal or memo built from a [`NodeSpec`], read the same way either way.
#[derive(Debug, Clone, Copy)]
pub enum Source {
  Signal(Signal<i64>),
  Memo(Memo<i64>),
}

impl Source {
  pub fn get(self) -> i64 {
    match self {
      Source::Signal(signal) => signal.get(),
      Source::Memo(memo) => memo.get(),
    }
  }
}

/// The real reactive nodes built from a [`GraphSpec`].
#[derive(Debug)]
pub struct Graph {
  pub scopes: Vec<Scope>,
  /// The signal or memo of each node, or `None` for effects.
  pub sources: Vec<Option<Source>>,
}

impl Graph {
  /// Builds every scope and node of `spec`, each node inside its own scope. `on_run` is called
  /// with the node index and the values an effect read, every time an effect runs.
  pub fn build(spec: &GraphSpec, on_run: impl Fn(usize, Vec<i64>) + Clone + 'static) -> Graph {
    let mut scopes: Vec<Scope> = Vec::new();
    for parent in &spec.scopes {
      scopes.push(match parent {
        None => Scope::new_root(),
        Some(parent) => scopes[*parent].child(),
      });
    }
    let mut sources: Vec<Option<Source>> = Vec::new();
    for (index, node) in spec.nodes.iter().enumerate() {
      let read = |inputs: &[usize]| -> Vec<Source> {
        inputs
          .iter()
          .map(|input| sources[*input].expect("inputs are signals or memos"))
          .collect()
      };
      let source = scopes[node.scope].run(|| match &node.kind {
        NodeKind::Signal { initial } => Some(Source::Signal(Signal::new(*initial))),
        NodeKind::Memo { inputs, modulus } => {
          let inputs = read(inputs);
          let modulus = *modulus;
          Some(Source::Memo(Memo::new(move || {
            inputs.iter().map(|input| input.get()).sum::<i64>() % modulus
          })))
        }
        NodeKind::Effect { inputs } => {
          let inputs = read(inputs);
          let on_run = on_run.clone();
          effect(move || on_run(index, inputs.iter().map(|input| input.get()).collect()));
          None
        }
      });
      sources.push(source);
    }
    Graph { scopes, sources }
  }

  pub fn root(&self) -> Scope {
    self.scopes[0]
  }
}
