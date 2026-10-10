use crate::Color;
use artic_core::tree::NodeId;
use artic_layout::geometry::{Rect, Transform};
use std::sync::Arc;

/// Something that changes how what's drawn inside it appears, until the matching pop.
///
/// Every renderer must support every variant, so new variants are breaking changes on purpose: a
/// renderer stops compiling instead of silently skipping what it doesn't draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Layer {
  /// Maps what's drawn inside, in its own coordinates, to the coordinates outside.
  Transform(Transform),
  /// Only draws inside the clip, in the coordinates the clip is pushed in.
  Clip(Clip),
  /// Draws everything inside as one group, then blends it at this opacity, from 0 to 1.
  Opacity(f32),
}

/// The shape a [`Layer::Clip`] keeps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Clip {
  /// A rectangle.
  Rect(Rect),
}

/// One recorded drawing command of a [`SceneFragment`].
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
  /// Starts a layer, ended by the matching [`Command::Pop`].
  Push(Layer),
  /// Ends the innermost layer.
  Pop,
  /// Fills `rect` with `color`.
  FillRect {
    /// The area to fill.
    rect: Rect,
    /// The color to fill it with.
    color: Color,
  },
}

/// One node's recorded drawing, in the node's own coordinates.
///
/// Its layers are balanced: every [`Command::Push`] has its [`Command::Pop`]. A renderer may cache
/// its own encoding of a fragment under its node and generation, since a fragment never changes;
/// recording the node again makes a new fragment with a higher generation.
#[derive(Debug, PartialEq)]
pub struct SceneFragment {
  node: NodeId,
  generation: u64,
  commands: Vec<Command>,
}

impl SceneFragment {
  pub(crate) fn new(node: NodeId, generation: u64, commands: Vec<Command>) -> SceneFragment {
    SceneFragment {
      node,
      generation,
      commands,
    }
  }

  /// The node that recorded it.
  #[must_use]
  pub fn node(&self) -> NodeId {
    self.node
  }

  /// Increases each time any node is recorded again, so `(node, generation)` identifies this
  /// recording.
  #[must_use]
  pub fn generation(&self) -> u64 {
    self.generation
  }

  /// The recorded commands, in order.
  #[must_use]
  pub fn commands(&self) -> &[Command] {
    &self.commands
  }
}

/// One item of a [`Scene`]: a node's layer, the end of one, or a node's fragment.
#[derive(Debug, Clone, PartialEq)]
pub enum SceneItem {
  /// Starts a node's layer, which also applies to its visual descendants.
  Push(Layer),
  /// Ends the innermost layer.
  Pop,
  /// A node's drawing, shared with the composition's cache rather than copied.
  Fragment(Arc<SceneFragment>),
}

/// One frame: the fragments of a visual subtree in drawing order, each wrapped in its nodes'
/// layers.
///
/// A renderer reads it from start to end. It's `Send`, so it can move to a render thread.
#[derive(Debug, Default, PartialEq)]
pub struct Scene {
  pub(crate) items: Vec<SceneItem>,
}

impl Scene {
  /// The items in drawing order. Layers are balanced.
  #[must_use]
  pub fn items(&self) -> &[SceneItem] {
    &self.items
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use static_assertions::assert_impl_all;

  assert_impl_all!(Scene: Send);
  assert_impl_all!(SceneFragment: Send, Sync);
}
