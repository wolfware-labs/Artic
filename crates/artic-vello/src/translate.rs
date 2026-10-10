use artic_layout::geometry::{Rect, Transform};
use artic_render::{Clip, Color, Command, Layer, PhysicalSize, Scene, SceneItem};
use vello::kurbo::{self, Affine};
use vello::peniko::{self, BlendMode, Fill};

/// Writes `scene` into `out`, drawn at `scale` device pixels per logical unit on a surface of
/// `size`.
pub(crate) fn translate(scene: &Scene, scale: f32, size: PhysicalSize, out: &mut vello::Scene) {
  out.reset();
  let mut translation = Translation {
    out,
    current: Affine::scale(f64::from(scale)),
    stack: Vec::new(),
    surface: kurbo::Rect::new(0.0, 0.0, f64::from(size.width), f64::from(size.height)),
  };
  for item in scene.items() {
    match item {
      SceneItem::Push(layer) => translation.push(*layer),
      SceneItem::Pop => translation.pop(),
      SceneItem::Fragment(fragment) => {
        for command in fragment.commands() {
          match command {
            Command::Push(layer) => translation.push(*layer),
            Command::Pop => translation.pop(),
            Command::FillRect { rect, color } => translation.fill_rect(*rect, *color),
          }
        }
      }
    }
  }
}

/// What a push left to undo at its pop.
enum Pushed {
  /// The transform in effect before it.
  Transform(Affine),
  /// A vello layer.
  Layer,
}

struct Translation<'a> {
  out: &'a mut vello::Scene,
  /// Maps the current coordinates to device pixels.
  current: Affine,
  stack: Vec<Pushed>,
  /// The whole surface in device pixels, the shape of an opacity layer.
  surface: kurbo::Rect,
}

impl Translation<'_> {
  fn push(&mut self, layer: Layer) {
    match layer {
      Layer::Transform(transform) => {
        self.stack.push(Pushed::Transform(self.current));
        self.current *= affine(transform);
      }
      Layer::Clip(Clip::Rect(rect)) => {
        self.out.push_clip_layer(Fill::NonZero, self.current, &kurbo_rect(rect));
        self.stack.push(Pushed::Layer);
      }
      Layer::Opacity(opacity) => {
        self
          .out
          .push_layer(Fill::NonZero, BlendMode::default(), opacity, Affine::IDENTITY, &self.surface);
        self.stack.push(Pushed::Layer);
      }
    }
  }

  fn pop(&mut self) {
    match self.stack.pop() {
      Some(Pushed::Transform(previous)) => self.current = previous,
      Some(Pushed::Layer) => self.out.pop_layer(),
      None => {}
    }
  }

  fn fill_rect(&mut self, rect: Rect, color: Color) {
    self
      .out
      .fill(Fill::NonZero, self.current, peniko_color(color), None, &kurbo_rect(rect));
  }
}

/// The same transform in kurbo's coefficient order, which is also CSS's.
pub(crate) fn affine(transform: Transform) -> Affine {
  let Transform { xx, yx, xy, yy, dx, dy } = transform;
  Affine::new([xx, yx, xy, yy, dx, dy].map(f64::from))
}

fn kurbo_rect(rect: Rect) -> kurbo::Rect {
  kurbo::Rect::from_origin_size(
    (f64::from(rect.x), f64::from(rect.y)),
    (f64::from(rect.width), f64::from(rect.height)),
  )
}

fn peniko_color(color: Color) -> peniko::Color {
  peniko::Color::from_rgba8(color.r, color.g, color.b, color.a)
}

#[cfg(test)]
mod tests {
  use super::*;
  use artic_layout::geometry::Point;
  use pretty_assertions::assert_eq;

  // artic's transform and kurbo's map points the same way.
  #[test]
  fn affine_matches_transform() {
    let transform = Transform::scale(2.0, 3.0)
      .then(Transform::rotate(0.5))
      .then(Transform::translate(4.0, -1.0));
    let point = Point::new(1.5, -2.0);

    let ours = transform.apply(point);
    let theirs = affine(transform) * kurbo::Point::new(1.5, -2.0);

    assert!((f64::from(ours.x) - theirs.x).abs() < 1e-5, "{ours} vs {theirs:?}");
    assert!((f64::from(ours.y) - theirs.y).abs() < 1e-5, "{ours} vs {theirs:?}");
    assert_eq!(affine(Transform::IDENTITY), Affine::IDENTITY);
  }
}
