use crate::{Brush, Clip, Command, Layer};
use artic_layout::geometry::{Rect, Transform};

/// Records one node's drawing, in the node's own coordinates.
///
/// Layers are only opened by the `with_*` methods, which close them when their closure returns, so
/// a recording can't leave a layer open for the nodes drawn after it.
///
/// # Examples
///
/// ```ignore
/// builder.fill_rect(bounds, Color::WHITE);
/// builder.with_clip(Clip::Rect(bounds), |builder| {
///   builder.with_opacity(0.5, |builder| builder.fill_rect(inner, Color::BLACK));
/// });
/// ```
#[derive(Debug, Default)]
pub struct SceneBuilder {
  commands: Vec<Command>,
}

impl SceneBuilder {
  /// Fills `rect` with `brush`, or with anything that converts into one, such as a
  /// [`Color`](crate::Color).
  pub fn fill_rect(&mut self, rect: Rect, brush: impl Into<Brush>) {
    self.commands.push(Command::FillRect {
      rect,
      brush: brush.into(),
    });
  }

  /// Records `draw` inside `transform`.
  pub fn with_transform(&mut self, transform: Transform, draw: impl FnOnce(&mut SceneBuilder)) {
    self.with_layer(Layer::Transform(transform), draw);
  }

  /// Records `draw` inside `clip`.
  pub fn with_clip(&mut self, clip: Clip, draw: impl FnOnce(&mut SceneBuilder)) {
    self.with_layer(Layer::Clip(clip), draw);
  }

  /// Records `draw` as one group drawn at `opacity`, from 0 to 1.
  pub fn with_opacity(&mut self, opacity: f32, draw: impl FnOnce(&mut SceneBuilder)) {
    self.with_layer(Layer::Opacity(opacity), draw);
  }

  /// Takes the recorded commands, leaving the builder empty.
  pub(crate) fn take(&mut self) -> Vec<Command> {
    std::mem::take(&mut self.commands)
  }

  fn with_layer(&mut self, layer: Layer, draw: impl FnOnce(&mut SceneBuilder)) {
    self.commands.push(Command::Push(layer));
    draw(self);
    self.commands.push(Command::Pop);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Color;
  use pretty_assertions::assert_eq;

  // Fills record in order, and each layer wraps exactly what its closure draws.
  #[test]
  fn layers_wrap_what_their_closure_draws() {
    let bounds = Rect::new(0.0, 0.0, 10.0, 10.0);
    let black = Color::rgb(0, 0, 0);
    let mut builder = SceneBuilder::default();

    builder.fill_rect(bounds, black);
    builder.with_clip(Clip::Rect(bounds), |builder| {
      builder.with_opacity(0.5, |builder| builder.fill_rect(bounds, black));
    });

    assert_eq!(
      builder.take(),
      [
        Command::FillRect {
          rect: bounds,
          brush: black.into()
        },
        Command::Push(Layer::Clip(Clip::Rect(bounds))),
        Command::Push(Layer::Opacity(0.5)),
        Command::FillRect {
          rect: bounds,
          brush: black.into()
        },
        Command::Pop,
        Command::Pop,
      ]
    );
    assert!(builder.take().is_empty(), "take leaves the builder empty");
  }
}
