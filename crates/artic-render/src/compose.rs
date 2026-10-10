use crate::{Clip, Layer, Scene, SceneBuilder, SceneFragment, SceneItem};
use artic_core::property::Affects;
use artic_core::tree::{NodeId, Tree, TreeArena};
use artic_layout::geometry::Transform;
use std::collections::HashMap;
use std::sync::Arc;

/// A node's transform, clip and opacity, which apply to it and to its visual descendants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layers {
  /// Maps the node's coordinates to its parent's, if they differ.
  pub transform: Option<Transform>,
  /// Keeps the node and its descendants inside this shape, in the node's coordinates.
  pub clip: Option<Clip>,
  /// Blends the node and its descendants as one group, from 0 to 1.
  pub opacity: f32,
}

impl Default for Layers {
  fn default() -> Layers {
    Layers {
      transform: None,
      clip: None,
      opacity: 1.0,
    }
  }
}

/// What composition asks about each node. The frame pipeline answers it from the elements; tests
/// answer it directly.
pub trait Nodes {
  /// The node's layers.
  fn layers(&self, arena: &TreeArena, node: NodeId) -> Layers;

  /// Records the node's own drawing, in its own coordinates, without its descendants.
  fn record(&mut self, arena: &TreeArena, node: NodeId, builder: &mut SceneBuilder);
}

/// Builds frames from the visual tree, keeping each node's fragment until the node is render-dirty
/// again.
#[derive(Debug, Default)]
pub struct Compositor {
  fragments: HashMap<NodeId, Arc<SceneFragment>>,
  generation: u64,
}

impl Compositor {
  /// Builds the frame for `root`'s visual subtree.
  ///
  /// Each node is wrapped in its layers (transform, then clip, then opacity, each only if set),
  /// which also wrap its visual descendants; it draws before them. A node is recorded again only if
  /// it's render-dirty or has no fragment yet, and its render flag is cleared. Fragments of nodes
  /// no longer in the subtree are dropped.
  pub fn compose(&mut self, arena: &mut TreeArena, root: NodeId, nodes: &mut impl Nodes) -> Scene {
    let mut previous = std::mem::take(&mut self.fragments);
    let mut walk = Walk {
      compositor: self,
      previous: &mut previous,
      nodes,
      builder: SceneBuilder::default(),
      items: Vec::new(),
    };
    walk.visit(arena, root);
    Scene { items: walk.items }
  }
}

/// One composition in progress.
struct Walk<'a, N> {
  compositor: &'a mut Compositor,
  previous: &'a mut HashMap<NodeId, Arc<SceneFragment>>,
  nodes: &'a mut N,
  builder: SceneBuilder,
  items: Vec<SceneItem>,
}

impl<N: Nodes> Walk<'_, N> {
  fn visit(&mut self, arena: &mut TreeArena, node: NodeId) {
    let layers = self.nodes.layers(arena, node);
    let set = [
      layers.transform.map(Layer::Transform),
      layers.clip.map(Layer::Clip),
      (layers.opacity < 1.0).then_some(Layer::Opacity(layers.opacity)),
    ];
    let mut pushed = 0;
    for layer in set.into_iter().flatten() {
      self.items.push(SceneItem::Push(layer));
      pushed += 1;
    }

    let fragment = match self.previous.remove(&node) {
      Some(fragment) if !arena.dirty(node).contains(Affects::RENDER) => fragment,
      _ => self.record(arena, node),
    };
    arena.clear_dirty(node, Affects::RENDER);
    self.compositor.fragments.insert(node, Arc::clone(&fragment));
    self.items.push(SceneItem::Fragment(fragment));

    let children: Vec<NodeId> = arena.children(Tree::Visual, node).collect();
    for child in children {
      self.visit(arena, child);
    }
    self.items.extend((0..pushed).map(|_| SceneItem::Pop));
  }

  fn record(&mut self, arena: &TreeArena, node: NodeId) -> Arc<SceneFragment> {
    self.nodes.record(arena, node, &mut self.builder);
    self.compositor.generation += 1;
    Arc::new(SceneFragment::new(node, self.compositor.generation, self.builder.take()))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{Color, Command};
  use artic_core::Object;
  use artic_core::property::AnyProperty;
  use artic_layout::geometry::Rect;
  use pretty_assertions::assert_eq;

  #[derive(Debug)]
  struct Node;

  impl Object for Node {
    fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  }

  /// Answers composition from a table, and counts how often each node is recorded.
  #[derive(Default)]
  struct Table {
    layers: HashMap<NodeId, Layers>,
    recorded: HashMap<NodeId, usize>,
  }

  impl Nodes for Table {
    fn layers(&self, _arena: &TreeArena, node: NodeId) -> Layers {
      self.layers.get(&node).copied().unwrap_or_default()
    }

    fn record(&mut self, _arena: &TreeArena, node: NodeId, builder: &mut SceneBuilder) {
      *self.recorded.entry(node).or_default() += 1;
      builder.fill_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0, 0, 0));
    }
  }

  /// A visual tree `root -> [a -> [c], b]`, returned as `[root, a, b, c]`.
  fn tree(arena: &mut TreeArena) -> [NodeId; 4] {
    let [root, a, b, c] = [(); 4].map(|()| arena.alloc(Box::new(Node)));
    arena.add_child(Tree::Visual, root, a);
    arena.add_child(Tree::Visual, root, b);
    arena.add_child(Tree::Visual, a, c);
    [root, a, b, c]
  }

  /// The nodes whose fragments `scene` draws, in order.
  fn fragment_nodes(scene: &Scene) -> Vec<NodeId> {
    scene
      .items()
      .iter()
      .filter_map(|item| match item {
        SceneItem::Fragment(fragment) => Some(fragment.node()),
        _ => None,
      })
      .collect()
  }

  // Fragments follow the visual tree depth first, each node before its descendants.
  #[test]
  fn scene_composes_fragments_in_visual_order() {
    let mut arena = TreeArena::new();
    let [root, a, b, c] = tree(&mut arena);

    let scene = Compositor::default().compose(&mut arena, root, &mut Table::default());

    assert_eq!(fragment_nodes(&scene), [root, a, c, b]);
  }

  // A node that isn't render-dirty keeps its fragment, shared rather than recorded again; a
  // render-dirty one is recorded again with a higher generation, and its flag is cleared.
  #[test]
  fn clean_fragment_is_reused() {
    let mut arena = TreeArena::new();
    let [root, a, b, c] = tree(&mut arena);
    let mut compositor = Compositor::default();
    let mut nodes = Table::default();
    let first = compositor.compose(&mut arena, root, &mut nodes);

    arena.mark_dirty(b, Affects::RENDER);
    let second = compositor.compose(&mut arena, root, &mut nodes);

    let recorded: Vec<usize> = [root, a, b, c].iter().map(|node| nodes.recorded[node]).collect();
    assert_eq!(recorded, [1, 1, 2, 1]);
    let fragment = |scene: &Scene, index: usize| match &scene.items()[index] {
      SceneItem::Fragment(fragment) => Arc::clone(fragment),
      other => panic!("expected a fragment, found {other:?}"),
    };
    assert!(Arc::ptr_eq(&fragment(&first, 0), &fragment(&second, 0)), "root is shared");
    assert!(fragment(&second, 3).generation() > fragment(&first, 3).generation());
    assert!(!arena.dirty(b).contains(Affects::RENDER));
  }

  // A node's transform, clip and opacity are pushed in that order around it and its descendants.
  #[test]
  fn composition_applies_transform_clip_opacity() {
    let mut arena = TreeArena::new();
    let [root, a, b, c] = tree(&mut arena);
    let names = HashMap::from([(root, "root"), (a, "a"), (b, "b"), (c, "c")]);
    let transform = Transform::translate(5.0, 0.0);
    let clip = Clip::Rect(Rect::new(0.0, 0.0, 4.0, 4.0));
    let mut nodes = Table::default();
    let layers = Layers {
      transform: Some(transform),
      clip: Some(clip),
      opacity: 0.5,
    };
    nodes.layers.insert(a, layers);

    let scene = Compositor::default().compose(&mut arena, root, &mut nodes);

    let shape: Vec<String> = scene
      .items()
      .iter()
      .map(|item| match item {
        SceneItem::Push(layer) => format!("push {layer:?}"),
        SceneItem::Pop => "pop".to_owned(),
        SceneItem::Fragment(fragment) => format!("fragment {}", names[&fragment.node()]),
      })
      .collect();
    let push = |layer: Layer| format!("push {layer:?}");
    assert_eq!(
      shape,
      [
        "fragment root".to_owned(),
        push(Layer::Transform(transform)),
        push(Layer::Clip(clip)),
        push(Layer::Opacity(0.5)),
        "fragment a".to_owned(),
        "fragment c".to_owned(),
        "pop".to_owned(),
        "pop".to_owned(),
        "pop".to_owned(),
        "fragment b".to_owned(),
      ]
    );
  }

  // A node that leaves the subtree loses its fragment, so freed nodes don't leak.
  #[test]
  fn removed_nodes_lose_their_fragments() {
    let mut arena = TreeArena::new();
    let [root, a, _, _] = tree(&mut arena);
    let mut compositor = Compositor::default();
    compositor.compose(&mut arena, root, &mut Table::default());

    arena.detach(Tree::Visual, a);
    compositor.compose(&mut arena, root, &mut Table::default());

    assert_eq!(compositor.fragments.len(), 2, "root and b");
  }

  // A recorded fragment holds what the node drew.
  #[test]
  fn fragments_hold_the_recording() {
    let mut arena = TreeArena::new();
    let root = arena.alloc(Box::new(Node));

    let scene = Compositor::default().compose(&mut arena, root, &mut Table::default());

    let SceneItem::Fragment(fragment) = &scene.items()[0] else {
      panic!("a fragment");
    };
    assert_eq!(
      fragment.commands(),
      [Command::FillRect {
        rect: Rect::new(0.0, 0.0, 1.0, 1.0),
        color: Color::rgb(0, 0, 0)
      }]
    );
  }
}
