use crate::reactive::runtime::Node;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, DefaultHasher};

#[derive(Default)]
struct Edges {
  sources: Vec<Node>,
  subscribers: Vec<Node>,
}

pub struct DependencyGraph {
  edges: RefCell<HashMap<Node, Edges, BuildHasherDefault<DefaultHasher>>>,
}

impl DependencyGraph {
  pub const fn new() -> Self {
    Self {
      edges: RefCell::new(HashMap::with_hasher(BuildHasherDefault::new())),
    }
  }

  pub fn add_edge(&self, source: Node, subscriber: Node) {
    let mut edges = self.edges.borrow_mut();
    let subscribers = &mut edges.entry(source).or_default().subscribers;
    if subscribers.contains(&subscriber) {
      return;
    }
    subscribers.push(subscriber);
    edges.entry(subscriber).or_default().sources.push(source);
  }

  pub fn source_at(&self, node: Node, index: usize) -> Option<Node> {
    self.edges.borrow().get(&node)?.sources.get(index).copied()
  }

  pub fn with_subscribers(&self, node: Node, action: impl FnOnce(&[Node])) {
    let edges = self.edges.borrow();
    action(edges.get(&node).map_or(&[], |edges| edges.subscribers.as_slice()));
  }

  pub fn clear_sources(&self, node: Node) {
    let mut edges = self.edges.borrow_mut();
    let Some(node_edges) = edges.get_mut(&node) else {
      return;
    };
    let sources = std::mem::take(&mut node_edges.sources);
    for source in sources {
      if let Some(source_edges) = edges.get_mut(&source) {
        source_edges.subscribers.retain(|subscriber| *subscriber != node);
      }
      Self::drop_if_empty(&mut edges, source);
    }
    Self::drop_if_empty(&mut edges, node);
  }

  pub fn remove(&self, node: Node) {
    self.clear_sources(node);
    let mut edges = self.edges.borrow_mut();
    let Some(node_edges) = edges.remove(&node) else {
      return;
    };
    for subscriber in node_edges.subscribers {
      if let Some(subscriber_edges) = edges.get_mut(&subscriber) {
        subscriber_edges.sources.retain(|source| *source != node);
      }
      Self::drop_if_empty(&mut edges, subscriber);
    }
  }

  fn drop_if_empty(edges: &mut HashMap<Node, Edges, BuildHasherDefault<DefaultHasher>>, node: Node) {
    if edges
      .get(&node)
      .is_some_and(|node_edges| node_edges.sources.is_empty() && node_edges.subscribers.is_empty())
    {
      edges.remove(&node);
    }
  }
}

#[cfg(test)]
mod tests {
  // These tests inspect the private edge map, which is only visible from this module.
  use super::*;
  use crate::reactive::Signal;
  use crate::reactive::runtime::Runtime;
  use pretty_assertions::assert_eq;

  /// Returns a signal node and two observer nodes to wire together.
  fn nodes() -> (Node, Node, Node) {
    let signal = Node::Signal(Signal::new(0).id());
    (signal, Runtime::create_observer(), Runtime::create_observer())
  }

  /// Returns the subscribers of `node`, or nothing if it has no entry.
  fn subscribers(graph: &DependencyGraph, node: Node) -> Vec<Node> {
    let mut found = Vec::new();
    graph.with_subscribers(node, |subscribers| found = subscribers.to_vec());
    found
  }

  // Adding the same edge twice keeps a single entry on each side.
  #[test]
  fn add_edge_ignores_duplicates() {
    let graph = DependencyGraph::new();
    let (signal, reader, _) = nodes();

    graph.add_edge(signal, reader);
    graph.add_edge(signal, reader);

    assert_eq!(subscribers(&graph, signal), [reader]);
    assert_eq!(graph.source_at(reader, 0), Some(signal));
    assert_eq!(graph.source_at(reader, 1), None);
  }

  // Clearing a node's sources removes it from each source's subscribers, and drops entries that are
  // left empty.
  #[test]
  fn clear_sources_cuts_both_directions() {
    let graph = DependencyGraph::new();
    let (signal, first, second) = nodes();
    graph.add_edge(signal, first);
    graph.add_edge(signal, second);

    graph.clear_sources(first);

    assert_eq!(subscribers(&graph, signal), [second]);
    assert_eq!(graph.source_at(first, 0), None);
    assert!(!graph.edges.borrow().contains_key(&first));
  }

  // Removing a node cuts its edges on both sides: its sources forget it, and so do its subscribers.
  #[test]
  fn remove_cuts_every_edge_of_the_node() {
    let graph = DependencyGraph::new();
    let (signal, middle, reader) = nodes();
    graph.add_edge(signal, middle);
    graph.add_edge(middle, reader);

    graph.remove(middle);

    assert_eq!(subscribers(&graph, signal), []);
    assert_eq!(graph.source_at(reader, 0), None);
    assert!(graph.edges.borrow().is_empty());
  }
}
