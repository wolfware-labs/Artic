use crate::{Clip, Layer, Scene, SceneBuilder, SceneFragment, SceneItem};
use artic_core::property::Affects;
use artic_core::tree::{NodeId, TreeKind, UiTree};
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
  /// Whether the node and its descendants are drawn at all.
  pub visible: bool,
  /// The node's drawing order among its siblings: lower first, ties in tree order.
  pub z_index: i32,
}

impl Default for Layers {
  fn default() -> Layers {
    Layers {
      transform: None,
      clip: None,
      opacity: 1.0,
      visible: true,
      z_index: 0,
    }
  }
}

/// What composition asks about each node. The frame pipeline answers it from the elements; tests
/// answer it directly.
pub trait Nodes {
  /// The node's layers.
  fn layers(&self, tree: &UiTree, node: NodeId) -> Layers;

  /// Records the node's own drawing, in its own coordinates, without its descendants.
  fn record(&mut self, tree: &UiTree, node: NodeId, builder: &mut SceneBuilder);
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
  /// which also wrap its visual descendants; it draws before them, and they draw in `z_index`
  /// order, ties in tree order. A node that isn't visible is skipped with its whole subtree. A
  /// node is recorded again only if it's render-dirty or has no fragment yet, and its render flag
  /// is cleared. Fragments of nodes no longer drawn are dropped.
  pub fn compose(&mut self, tree: &mut UiTree, root: NodeId, nodes: &mut impl Nodes) -> Scene {
    let mut previous = std::mem::take(&mut self.fragments);
    let mut walk = Walk {
      compositor: self,
      previous: &mut previous,
      nodes,
      builder: SceneBuilder::default(),
      items: Vec::new(),
    };
    walk.visit(tree, root);
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
  fn visit(&mut self, tree: &mut UiTree, node: NodeId) {
    let layers = self.nodes.layers(tree, node);
    if !layers.visible {
      return;
    }
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
      Some(fragment) if !tree.dirty(node).contains(Affects::RENDER) => fragment,
      _ => self.record(tree, node),
    };
    tree.clear_dirty(node, Affects::RENDER);
    self.compositor.fragments.insert(node, Arc::clone(&fragment));
    self.items.push(SceneItem::Fragment(fragment));

    let mut children: Vec<(i32, NodeId)> = tree
      .children(TreeKind::Visual, node)
      .map(|child| (self.nodes.layers(tree, child).z_index, child))
      .collect();
    children.sort_by_key(|(z_index, _)| *z_index);
    for (_, child) in children {
      self.visit(tree, child);
    }
    self.items.extend((0..pushed).map(|_| SceneItem::Pop));
  }

  fn record(&mut self, tree: &UiTree, node: NodeId) -> Arc<SceneFragment> {
    self.nodes.record(tree, node, &mut self.builder);
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
    fn layers(&self, _arena: &UiTree, node: NodeId) -> Layers {
      self.layers.get(&node).copied().unwrap_or_default()
    }

    fn record(&mut self, _arena: &UiTree, node: NodeId, builder: &mut SceneBuilder) {
      *self.recorded.entry(node).or_default() += 1;
      builder.fill_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0, 0, 0));
    }
  }

  /// A visual tree `root -> [a -> [c], b]`, returned as `[root, a, b, c]`.
  fn sample(tree: &mut UiTree) -> [NodeId; 4] {
    let [root, a, b, c] = [(); 4].map(|()| tree.alloc(Box::new(Node)));
    tree.add_child(TreeKind::Visual, root, a);
    tree.add_child(TreeKind::Visual, root, b);
    tree.add_child(TreeKind::Visual, a, c);
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

  // A node that isn't visible is skipped together with its whole subtree, visible descendants
  // included.
  #[test]
  fn hidden_node_skips_its_subtree() {
    let mut tree = UiTree::new();
    let [root, a, b, _c] = sample(&mut tree);
    let mut nodes = Table::default();
    nodes.layers.insert(
      a,
      Layers {
        visible: false,
        ..Layers::default()
      },
    );

    let scene = Compositor::default().compose(&mut tree, root, &mut nodes);

    assert_eq!(fragment_nodes(&scene), [root, b]);
  }

  // A subtree hidden for a frame comes back whole once it's visible again.
  #[test]
  fn subtree_returns_when_visible_again() {
    let mut tree = UiTree::new();
    let [root, a, b, c] = sample(&mut tree);
    let mut compositor = Compositor::default();
    let mut nodes = Table::default();
    nodes.layers.insert(
      a,
      Layers {
        visible: false,
        ..Layers::default()
      },
    );
    compositor.compose(&mut tree, root, &mut nodes);

    nodes.layers.remove(&a);
    let scene = compositor.compose(&mut tree, root, &mut nodes);

    assert_eq!(fragment_nodes(&scene), [root, a, c, b]);
  }

  // Siblings draw by ZIndex, lowest first; equal values keep tree order.
  #[test]
  fn children_draw_by_z_index() {
    let mut tree = UiTree::new();
    let root = tree.alloc(Box::new(Node));
    let [first, second, third] = [(); 3].map(|()| tree.alloc(Box::new(Node)));
    for child in [first, second, third] {
      tree.add_child(TreeKind::Visual, root, child);
    }
    let mut nodes = Table::default();
    nodes.layers.insert(
      first,
      Layers {
        z_index: 1,
        ..Layers::default()
      },
    );
    nodes.layers.insert(
      third,
      Layers {
        z_index: -1,
        ..Layers::default()
      },
    );

    let scene = Compositor::default().compose(&mut tree, root, &mut nodes);

    assert_eq!(fragment_nodes(&scene), [root, third, second, first]);
  }

  // Fragments follow the visual tree depth first, each node before its descendants.
  #[test]
  fn scene_composes_fragments_in_visual_order() {
    let mut tree = UiTree::new();
    let [root, a, b, c] = sample(&mut tree);

    let scene = Compositor::default().compose(&mut tree, root, &mut Table::default());

    assert_eq!(fragment_nodes(&scene), [root, a, c, b]);
  }

  // A node that isn't render-dirty keeps its fragment, shared rather than recorded again; a
  // render-dirty one is recorded again with a higher generation, and its flag is cleared.
  #[test]
  fn clean_fragment_is_reused() {
    let mut tree = UiTree::new();
    let [root, a, b, c] = sample(&mut tree);
    let mut compositor = Compositor::default();
    let mut nodes = Table::default();
    let first = compositor.compose(&mut tree, root, &mut nodes);

    tree.mark_dirty(b, Affects::RENDER);
    let second = compositor.compose(&mut tree, root, &mut nodes);

    let recorded: Vec<usize> = [root, a, b, c].iter().map(|node| nodes.recorded[node]).collect();
    assert_eq!(recorded, [1, 1, 2, 1]);
    let fragment = |scene: &Scene, index: usize| match &scene.items()[index] {
      SceneItem::Fragment(fragment) => Arc::clone(fragment),
      other => panic!("expected a fragment, found {other:?}"),
    };
    assert!(Arc::ptr_eq(&fragment(&first, 0), &fragment(&second, 0)), "root is shared");
    assert!(fragment(&second, 3).generation() > fragment(&first, 3).generation());
    assert!(!tree.dirty(b).contains(Affects::RENDER));
  }

  // A node's transform, clip and opacity are pushed in that order around it and its descendants.
  #[test]
  fn composition_applies_transform_clip_opacity() {
    let mut tree = UiTree::new();
    let [root, a, b, c] = sample(&mut tree);
    let names = HashMap::from([(root, "root"), (a, "a"), (b, "b"), (c, "c")]);
    let transform = Transform::translate(5.0, 0.0);
    let clip = Clip::Rect(Rect::new(0.0, 0.0, 4.0, 4.0));
    let mut nodes = Table::default();
    let layers = Layers {
      transform: Some(transform),
      clip: Some(clip),
      opacity: 0.5,
      ..Layers::default()
    };
    nodes.layers.insert(a, layers);

    let scene = Compositor::default().compose(&mut tree, root, &mut nodes);

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
    let mut tree = UiTree::new();
    let [root, a, _, _] = sample(&mut tree);
    let mut compositor = Compositor::default();
    compositor.compose(&mut tree, root, &mut Table::default());

    tree.detach(TreeKind::Visual, a);
    compositor.compose(&mut tree, root, &mut Table::default());

    assert_eq!(compositor.fragments.len(), 2, "root and b");
  }

  // A recorded fragment holds what the node drew.
  #[test]
  fn fragments_hold_the_recording() {
    let mut tree = UiTree::new();
    let root = tree.alloc(Box::new(Node));

    let scene = Compositor::default().compose(&mut tree, root, &mut Table::default());

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
