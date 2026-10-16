use artic_core::Object;
use artic_core::tree::{NodeId, TreeKind, UiTree};
use artic_layout::geometry::Size;
use artic_layout::{LayoutManager, LayoutNodes, LayoutStats};
use std::collections::HashMap;

/// A tree laid out with no window and no GPU. Nodes get a label when they're added, and failures
/// name them by it.
#[derive(Debug)]
pub struct Headless<N: LayoutNodes> {
  pub(super) tree: UiTree,
  pub(super) manager: LayoutManager,
  pub(super) nodes: N,
  pub(super) snap: bool,
  labels: HashMap<NodeId, String>,
}

impl<N: LayoutNodes> Headless<N> {
  /// An empty tree whose nodes `nodes` lays out, with snapping on.
  #[must_use]
  pub fn new(nodes: N) -> Headless<N> {
    Headless {
      tree: UiTree::new(),
      manager: LayoutManager::default(),
      nodes,
      snap: true,
      labels: HashMap::new(),
    }
  }

  /// Adds `object` as a node with no parent, labeled `label`.
  pub fn add(&mut self, label: &str, object: impl Object) -> NodeId {
    let node = self.tree.alloc(Box::new(object));
    self.labels.insert(node, label.to_owned());
    node
  }

  /// Adds `object` labeled `label`, with `children` as its visual children in order.
  pub fn parent(&mut self, label: &str, object: impl Object, children: &[NodeId]) -> NodeId {
    let node = self.add(label, object);
    for child in children {
      self.tree.add_child(TreeKind::Visual, node, *child);
    }
    node
  }

  /// The tree, for what `Headless` doesn't wrap.
  pub fn tree(&mut self) -> &mut UiTree {
    &mut self.tree
  }

  /// The layout manager, for what `Headless` doesn't wrap.
  pub fn manager(&mut self) -> &mut LayoutManager {
    &mut self.manager
  }

  /// Turns snapping to the device-pixel grid on or off for the next runs. It's on by default, as
  /// in a window.
  pub fn set_snap(&mut self, snap: bool) {
    self.snap = snap;
  }

  /// Lays out `root`'s subtree in `available`, at `scale` device pixels per logical pixel.
  pub fn run(&mut self, root: NodeId, available: Size, scale: f32) -> LayoutStats {
    self.manager.run(&mut self.tree, &self.nodes, root, available, scale, self.snap)
  }

  /// The label `node` was added with, or its id for a node made through [`tree`](Headless::tree).
  #[must_use]
  pub fn label(&self, node: NodeId) -> String {
    self.labels.get(&node).cloned().unwrap_or_else(|| format!("{node:?}"))
  }

  pub(super) fn name(&self, node: NodeId) -> String {
    match self.labels.get(&node) {
      Some(label) => format!("{label:?}"),
      None => format!("{node:?}"),
    }
  }
}
