use artic_layout::geometry::{CornerRadius, FillRule, PathElement, Point, Rect, Transform};
use artic_render::{Brush, Color, Command, GradientStop, Layer, PhysicalSize, Scene, SceneItem, Shape};
use vello::kurbo::{self, Affine, BezPath, PathEl, Shape as _};
use vello::peniko::{self, BlendMode, Fill, Gradient};

/// The tolerance vello flattens a shape with, so converting one to a path first changes nothing.
const TOLERANCE: f64 = 0.1;

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
      SceneItem::Push(layer) => translation.push(layer),
      SceneItem::Pop => translation.pop(),
      SceneItem::Fragment(fragment) => {
        for command in fragment.commands() {
          match command {
            Command::Push(layer) => translation.push(layer),
            Command::Pop => translation.pop(),
            Command::Fill { shape, brush } => translation.fill(shape, brush),
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
  fn push(&mut self, layer: &Layer) {
    match layer {
      Layer::Transform(transform) => {
        self.stack.push(Pushed::Transform(self.current));
        self.current *= affine(*transform);
      }
      Layer::Clip(shape) => {
        let (style, clip) = shape
          .drawable()
          .map_or((Fill::NonZero, BezPath::new()), |shape| (fill_rule(&shape), outline(&shape)));
        self.out.push_clip_layer(style, self.current, &clip);
        self.stack.push(Pushed::Layer);
      }
      Layer::Opacity(opacity) => {
        self
          .out
          .push_layer(Fill::NonZero, BlendMode::default(), *opacity, Affine::IDENTITY, &self.surface);
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

  fn fill(&mut self, shape: &Shape, brush: &Brush) {
    let Some(shape) = shape.drawable() else {
      return;
    };
    if let Some((brush, transform)) = paint(brush, shape.bounds()) {
      self
        .out
        .fill(fill_rule(&shape), self.current, &brush, transform, &outline(&shape));
    }
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

fn outline(shape: &Shape) -> BezPath {
  match shape {
    Shape::Rect(rect) => kurbo_rect(*rect).to_path(TOLERANCE),
    Shape::RoundedRect(rounded) => {
      kurbo::RoundedRect::from_rect(kurbo_rect(rounded.rect), radii(rounded.radius)).to_path(TOLERANCE)
    }
    Shape::Path(path) => path.elements().iter().map(|element| path_el(*element)).collect(),
  }
}

fn fill_rule(shape: &Shape) -> Fill {
  match shape {
    Shape::Path(path) => match path.fill_rule() {
      FillRule::EvenOdd => Fill::EvenOdd,
      FillRule::NonZero => Fill::NonZero,
    },
    Shape::Rect(_) | Shape::RoundedRect(_) => Fill::NonZero,
  }
}

fn path_el(element: PathElement) -> PathEl {
  match element {
    PathElement::MoveTo(point) => PathEl::MoveTo(kurbo_point(point)),
    PathElement::LineTo(point) => PathEl::LineTo(kurbo_point(point)),
    PathElement::QuadTo(control, end) => PathEl::QuadTo(kurbo_point(control), kurbo_point(end)),
    PathElement::CubicTo(first, second, end) => PathEl::CurveTo(kurbo_point(first), kurbo_point(second), kurbo_point(end)),
    PathElement::Close => PathEl::ClosePath,
  }
}

fn radii(radius: CornerRadius) -> kurbo::RoundedRectRadii {
  let CornerRadius {
    top_left,
    top_right,
    bottom_right,
    bottom_left,
  } = radius;
  kurbo::RoundedRectRadii::new(
    f64::from(top_left),
    f64::from(top_right),
    f64::from(bottom_right),
    f64::from(bottom_left),
  )
}

fn paint(brush: &Brush, rect: Rect) -> Option<(peniko::Brush, Option<Affine>)> {
  let (stops, opacity, gradient) = match brush {
    Brush::Solid(solid) => return Some((peniko::Brush::Solid(color(solid.color, solid.opacity)), None)),
    Brush::Linear(linear) => {
      let (start, end) = (kurbo_point(linear.start.resolve(rect)), kurbo_point(linear.end.resolve(rect)));
      let gradient = (start.is_finite() && end.is_finite() && start != end).then(|| (Gradient::new_linear(start, end), None));
      (&linear.stops, linear.opacity, gradient)
    }
    Brush::Radial(radial) => {
      let radius_x = radius(radial.radius_x.resolve(rect.width));
      let radius_y = radius(radial.radius_y.resolve(rect.height));
      let center = kurbo_point(radial.center.resolve(rect));
      let gradient = (center.is_finite() && radius_x > 0.0 && radius_y > 0.0).then(|| {
        let ellipse = Affine::translate(center.to_vec2()) * Affine::scale_non_uniform(f64::from(radius_x), f64::from(radius_y));
        (Gradient::new_radial((0.0, 0.0), 1.0), Some(ellipse))
      });
      (&radial.stops, radial.opacity, gradient)
    }
  };
  let stops = color_stops(stops, opacity);
  let (_, last) = *stops.last()?;
  match gradient {
    Some((gradient, transform)) if stops.len() > 1 => {
      Some((peniko::Brush::Gradient(gradient.with_stops(stops.as_slice())), transform))
    }
    _ => Some((peniko::Brush::Solid(last), None)),
  }
}

fn color_stops(stops: &[GradientStop], opacity: f32) -> Vec<(f32, peniko::Color)> {
  let mut stops: Vec<_> = stops
    .iter()
    .map(|stop| (unit(stop.offset), color(stop.color, opacity)))
    .collect();
  stops.sort_by(|(a, _), (b, _)| a.total_cmp(b));
  stops
}

fn color(color: Color, opacity: f32) -> peniko::Color {
  peniko::Color::from_rgba8(color.r, color.g, color.b, color.a).multiply_alpha(unit(opacity))
}

fn unit(value: f32) -> f32 {
  if value > 0.0 { value.min(1.0) } else { 0.0 }
}

fn radius(value: f32) -> f32 {
  if value > 0.0 && value.is_finite() { value } else { 0.0 }
}

fn kurbo_point(point: Point) -> kurbo::Point {
  kurbo::Point::new(f64::from(point.x), f64::from(point.y))
}

#[cfg(test)]
mod tests {
  use super::*;
  use artic_layout::geometry::{CornerRadius, FillRule, Path, Point, RelativePoint, RelativeScalar, RoundedRect};
  use artic_render::{LinearGradientBrush, RadialGradientBrush, SolidColorBrush};
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

  const RED: Color = Color::rgb(255, 0, 0);
  const GREEN: Color = Color::rgb(0, 255, 0);
  const BLUE: Color = Color::rgb(0, 0, 255);

  fn opaque(r: u8, g: u8, b: u8) -> peniko::Color {
    peniko::Color::from_rgba8(r, g, b, 255)
  }

  fn red_to_blue() -> [GradientStop; 2] {
    [GradientStop::new(0.0, RED), GradientStop::new(1.0, BLUE)]
  }

  // Each brush becomes the vello brush of the same colors, its points and radii resolved against
  // the rectangle and its opacity applied to every alpha.
  #[test]
  fn brush_to_peniko_roundtrip() {
    let rect = Rect::new(10.0, 20.0, 100.0, 50.0);
    let (red, blue) = (opaque(255, 0, 0), opaque(0, 0, 255));
    let solid = SolidColorBrush {
      color: RED,
      opacity: 0.5,
    };
    let linear = LinearGradientBrush {
      opacity: 0.5,
      ..LinearGradientBrush::new(
        RelativePoint::relative(0.0, 0.5),
        RelativePoint::fixed(100.0, 25.0),
        red_to_blue(),
      )
    };
    let radial = RadialGradientBrush::new(
      RelativePoint::relative(0.5, 0.5),
      RelativeScalar::relative(0.5),
      RelativeScalar::fixed(10.0),
      red_to_blue(),
    );

    assert_eq!(
      paint(&solid.into(), rect),
      Some((peniko::Brush::Solid(red.with_alpha(0.5)), None))
    );
    assert_eq!(
      paint(&linear.into(), rect),
      Some((
        peniko::Brush::Gradient(
          Gradient::new_linear((10.0, 45.0), (110.0, 45.0))
            .with_stops([(0.0, red.with_alpha(0.5)), (1.0, blue.with_alpha(0.5))])
        ),
        None
      ))
    );
    assert_eq!(
      paint(&radial.into(), rect),
      Some((
        peniko::Brush::Gradient(Gradient::new_radial((0.0, 0.0), 1.0).with_stops([(0.0, red), (1.0, blue)])),
        Some(Affine::translate((60.0, 45.0)) * Affine::scale_non_uniform(50.0, 10.0))
      ))
    );
  }

  // Offsets and opacity are clamped into 0..=1 and radii to 0 or more, NaN counting as 0, and
  // stops are sorted by offset, keeping their written order among equal offsets.
  #[test]
  fn gradient_values_are_clamped_and_sorted() {
    let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
    let [red, green, blue] = [opaque(255, 0, 0), opaque(0, 255, 0), opaque(0, 0, 255)];
    let unordered = [
      GradientStop::new(2.0, BLUE),
      GradientStop::new(f32::NAN, RED),
      GradientStop::new(-1.0, GREEN),
    ];
    let linear = |opacity| {
      Brush::from(LinearGradientBrush {
        opacity,
        ..LinearGradientBrush::new(
          RelativePoint::relative(0.0, 0.0),
          RelativePoint::relative(1.0, 0.0),
          unordered,
        )
      })
    };
    let expected = |alpha: f32| {
      Some((
        peniko::Brush::Gradient(Gradient::new_linear((0.0, 0.0), (10.0, 0.0)).with_stops([
          (0.0, red.with_alpha(alpha)),
          (0.0, green.with_alpha(alpha)),
          (1.0, blue.with_alpha(alpha)),
        ])),
        None,
      ))
    };

    assert_eq!(paint(&linear(2.0), rect), expected(1.0));
    assert_eq!(paint(&linear(f32::NAN), rect), expected(0.0));
    assert_eq!(paint(&linear(-1.0), rect), expected(0.0));
    for radius in [-1.0, f32::NAN] {
      let radial = RadialGradientBrush {
        radius_y: RelativeScalar::fixed(radius),
        stops: red_to_blue().into(),
        ..RadialGradientBrush::default()
      };
      assert_eq!(
        paint(&radial.into(), rect),
        Some((peniko::Brush::Solid(blue), None)),
        "radius {radius}"
      );
    }
  }

  // A gradient with nothing to spread over fills with its last color: a zero radius, a box with
  // no width, or a line whose ends meet. One stop fills with its color, and no stops paint nothing.
  #[test]
  fn degenerate_gradients_fill_with_their_last_color() {
    let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
    let blue = Some((peniko::Brush::Solid(opaque(0, 0, 255)), None));
    let radial = RadialGradientBrush {
      stops: red_to_blue().into(),
      ..RadialGradientBrush::default()
    };
    let zero = RadialGradientBrush {
      radius_x: RelativeScalar::fixed(0.0),
      ..radial.clone()
    };
    let meeting = LinearGradientBrush::new(
      RelativePoint::relative(0.5, 0.5),
      RelativePoint::fixed(5.0, 5.0),
      red_to_blue(),
    );
    let single = LinearGradientBrush::new(
      RelativePoint::relative(0.0, 0.0),
      RelativePoint::relative(1.0, 0.0),
      [GradientStop::new(0.3, BLUE)],
    );

    assert_eq!(paint(&zero.into(), rect), blue);
    assert_eq!(paint(&radial.into(), Rect::new(0.0, 0.0, 0.0, 10.0)), blue);
    assert_eq!(paint(&meeting.into(), rect), blue);
    assert_eq!(paint(&single.into(), rect), blue);
    assert_eq!(paint(&LinearGradientBrush::default().into(), rect), None);
  }

  // Geometry that isn't finite, which a brush built in code can carry, fills with the last color
  // too, instead of handing vello a NaN or infinite gradient.
  #[test]
  fn non_finite_gradients_fill_with_their_last_color() {
    let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
    let blue = Some((peniko::Brush::Solid(opaque(0, 0, 255)), None));
    let linear = |start, end| Brush::from(LinearGradientBrush::new(start, end, red_to_blue()));
    let radial = RadialGradientBrush {
      stops: red_to_blue().into(),
      ..RadialGradientBrush::default()
    };

    assert_eq!(
      paint(
        &linear(RelativePoint::relative(f32::NAN, 0.0), RelativePoint::relative(1.0, 0.0)),
        rect
      ),
      blue
    );
    assert_eq!(
      paint(
        &linear(RelativePoint::relative(0.0, 0.0), RelativePoint::fixed(f32::INFINITY, 0.0)),
        rect
      ),
      blue
    );
    assert_eq!(
      paint(
        &RadialGradientBrush {
          radius_x: RelativeScalar::fixed(f32::INFINITY),
          ..radial.clone()
        }
        .into(),
        rect
      ),
      blue
    );
    assert_eq!(
      paint(
        &RadialGradientBrush {
          center: RelativePoint::relative(0.5, f32::NAN),
          ..radial
        }
        .into(),
        rect
      ),
      blue
    );
  }

  fn encoded(draw: impl FnOnce(&mut Translation<'_>)) -> vello::Scene {
    let mut out = vello::Scene::new();
    draw(&mut Translation {
      out: &mut out,
      current: Affine::IDENTITY,
      stack: Vec::new(),
      surface: kurbo::Rect::ZERO,
    });
    out
  }

  // Each shape becomes the kurbo path vello would make of it: a rectangle, a rounded rectangle
  // with its own radius on each corner, or the path's elements one for one. A path keeps its
  // fill rule; rectangles fill by non-zero.
  #[test]
  fn shapes_convert_to_kurbo() {
    let rect = Rect::new(10.0, 20.0, 100.0, 50.0);
    let corners = kurbo::Rect::new(10.0, 20.0, 110.0, 70.0);
    let rounded = RoundedRect::new(rect, CornerRadius::new(1.0, 2.0, 3.0, 4.0));
    let path = Path::builder(Point::new(0.0, 0.0))
      .line_to(Point::new(10.0, 0.0))
      .quad_to(Point::new(10.0, 10.0), Point::new(0.0, 10.0))
      .cubic_to(Point::new(0.0, 5.0), Point::new(5.0, 5.0), Point::new(5.0, 0.0))
      .close()
      .fill_rule(FillRule::NonZero)
      .build();
    let mut expected = BezPath::new();
    expected.move_to((0.0, 0.0));
    expected.line_to((10.0, 0.0));
    expected.quad_to((10.0, 10.0), (0.0, 10.0));
    expected.curve_to((0.0, 5.0), (5.0, 5.0), (5.0, 0.0));
    expected.close_path();

    assert_eq!(outline(&rect.into()), corners.to_path(TOLERANCE));
    assert_eq!(
      outline(&rounded.into()),
      kurbo::RoundedRect::from_rect(corners, (1.0, 2.0, 3.0, 4.0)).to_path(TOLERANCE)
    );
    assert_eq!(outline(&path.clone().into()), expected);
    assert_eq!(
      [
        fill_rule(&rect.into()),
        fill_rule(&rounded.into()),
        fill_rule(&path.into()),
        fill_rule(&Path::builder(Point::ZERO).build().into()),
      ],
      [Fill::NonZero, Fill::NonZero, Fill::NonZero, Fill::EvenOdd]
    );
  }

  // A shape that draws nothing by the rules of `Shape::drawable` reaches vello as no path at all,
  // and so does a path that is only its start point.
  #[test]
  fn invalid_shapes_and_strokes_draw_nothing() {
    let red = Brush::from(RED);
    let invalid: [Shape; 5] = [
      Rect::new(f32::NAN, 0.0, 1.0, 1.0).into(),
      Rect::new(0.0, 0.0, f32::INFINITY, 1.0).into(),
      RoundedRect::new(Rect::new(0.0, 0.0, -1.0, 1.0), CornerRadius::ZERO).into(),
      Path::builder(Point::ZERO)
        .line_to(Point::new(f32::NEG_INFINITY, 1.0))
        .build()
        .into(),
      Path::builder(Point::ZERO).build().into(),
    ];

    for shape in &invalid {
      assert_eq!(encoded(|t| t.fill(shape, &red)).encoding().n_paths, 0, "{shape:?}");
    }
    assert_eq!(
      encoded(|t| t.fill(&Rect::new(0.0, 0.0, 1.0, 1.0).into(), &red))
        .encoding()
        .n_paths,
      1,
      "a valid fill draws"
    );
  }
}
