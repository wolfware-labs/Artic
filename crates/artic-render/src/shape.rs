use artic_layout::geometry::{Path, Rect, RoundedRect};

/// What a fill, a stroke or a clip covers.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
  /// A rectangle.
  Rect(Rect),
  /// A rectangle with rounded corners.
  RoundedRect(RoundedRect),
  /// Any outline of lines and curves.
  Path(Path),
}

impl Shape {
  /// The box a brush's relative points and radii resolve against: the rectangle, or the path's
  /// exact [bounds](Path::bounds).
  #[must_use]
  pub fn bounds(&self) -> Rect {
    match self {
      Shape::Rect(rect) => *rect,
      Shape::RoundedRect(rounded) => rounded.rect,
      Shape::Path(path) => path.bounds(),
    }
  }

  /// The shape as it's drawn, or `None` when it draws nothing: it has a NaN or infinite
  /// coordinate, or it's a rectangle with a negative width or height. A rounded rectangle's radii
  /// are [fitted](RoundedRect::fitted).
  #[must_use]
  pub fn drawable(&self) -> Option<Shape> {
    match self {
      Shape::Rect(rect) => drawable_rect(*rect).map(Shape::Rect),
      Shape::RoundedRect(rounded) => drawable_rect(rounded.rect).map(|_| Shape::RoundedRect(rounded.fitted())),
      Shape::Path(path) => path.is_finite().then(|| Shape::Path(path.clone())),
    }
  }
}

impl From<Rect> for Shape {
  fn from(rect: Rect) -> Shape {
    Shape::Rect(rect)
  }
}

impl From<RoundedRect> for Shape {
  fn from(rounded: RoundedRect) -> Shape {
    Shape::RoundedRect(rounded)
  }
}

impl From<Path> for Shape {
  fn from(path: Path) -> Shape {
    Shape::Path(path)
  }
}

fn drawable_rect(rect: Rect) -> Option<Rect> {
  let Rect { x, y, width, height } = rect;
  let finite = [x, y, width, height].iter().all(|value| value.is_finite());
  (finite && width >= 0.0 && height >= 0.0).then_some(rect)
}

#[cfg(test)]
mod tests {
  use super::*;
  use artic_layout::geometry::{CornerRadius, Point};
  use pretty_assertions::assert_eq;

  // A rectangle is its own box, a rounded one its rectangle's, and a path its exact bounds.
  #[test]
  fn shape_bounds_cover_each_variant() {
    let rect = Rect::new(1.0, 2.0, 3.0, 4.0);
    let arch = Path::builder(Point::new(0.0, 0.0))
      .quad_to(Point::new(5.0, 10.0), Point::new(10.0, 0.0))
      .build();

    assert_eq!(Shape::from(rect).bounds(), rect);
    assert_eq!(Shape::from(RoundedRect::new(rect, CornerRadius::uniform(1.0))).bounds(), rect);
    assert_eq!(Shape::from(arch).bounds(), Rect::new(0.0, 0.0, 10.0, 5.0));
  }

  // A shape with a NaN or infinite coordinate, or a negative width or height, draws nothing; a
  // drawable rounded rectangle has its radii fitted.
  #[test]
  fn shape_drawable_follows_the_rules() {
    let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
    let rounded = |rect| Shape::from(RoundedRect::new(rect, CornerRadius::ZERO));
    let broken_path = Path::builder(Point::ZERO).line_to(Point::new(f32::NAN, 1.0)).build();

    for shape in [
      Shape::from(Rect::new(f32::NAN, 0.0, 1.0, 1.0)),
      Shape::from(Rect::new(0.0, 0.0, f32::INFINITY, 1.0)),
      Shape::from(Rect::new(0.0, 0.0, -1.0, 1.0)),
      rounded(Rect::new(0.0, f32::NEG_INFINITY, 1.0, 1.0)),
      rounded(Rect::new(0.0, 0.0, 1.0, -1.0)),
      Shape::from(broken_path),
    ] {
      assert_eq!(shape.drawable(), None, "{shape:?}");
    }
    assert_eq!(Shape::from(rect).drawable(), Some(Shape::from(rect)));
    assert_eq!(
      Shape::from(RoundedRect::new(rect, CornerRadius::uniform(10.0))).drawable(),
      Some(Shape::from(RoundedRect::new(rect, CornerRadius::uniform(5.0))))
    );
  }
}
