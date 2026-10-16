//! One scene per drawing primitive, the same for every renderer, so each renderer keeps goldens
//! of the same pictures.

use artic_core::Object;
use artic_core::property::AnyProperty;
use artic_core::tree::{NodeId, UiTree};
use artic_render::{Compositor, Layers, Nodes, Scene, SceneBuilder};

/// The scene `draw` records, as one node of a tree.
pub fn scene(draw: impl FnOnce(&mut SceneBuilder)) -> Scene {
  let mut tree = UiTree::new();
  let root = tree.alloc(Box::new(Node));
  Compositor::default().compose(&mut tree, root, &mut Drawing(Some(draw)))
}

#[derive(Debug)]
struct Node;

impl Object for Node {
  fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
}

struct Drawing<F>(Option<F>);

impl<F: FnOnce(&mut SceneBuilder)> Nodes for Drawing<F> {
  fn layers(&self, _arena: &UiTree, _node: NodeId) -> Layers {
    Layers::default()
  }

  fn record(&mut self, _arena: &UiTree, _node: NodeId, builder: &mut SceneBuilder) {
    if let Some(draw) = self.0.take() {
      draw(builder);
    }
  }
}
