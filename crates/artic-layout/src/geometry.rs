//! The value types layout measures and positions with, in logical pixels.

mod parse;
mod path;
mod relative;
mod transform;

pub use parse::{GeometryProblem, ParseGeometryError};
pub use path::{FillRule, Path, PathBuilder, PathElement};
pub use relative::{RelativePoint, RelativeScalar, Scaling};
use std::fmt::{Display, Formatter, Result};
use std::ops::{Add, Sub};
pub use transform::Transform;

/// A width and a height, without a position.
///
/// `Size::INFINITY` stands for "no limit" in a measure constraint.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Size {
  /// The horizontal extent.
  pub width: f32,
  /// The vertical extent.
  pub height: f32,
}

impl Size {
  /// No width and no height.
  pub const ZERO: Size = Size::new(0.0, 0.0);
  /// Unlimited in both directions, for measure constraints.
  pub const INFINITY: Size = Size::new(f32::INFINITY, f32::INFINITY);

  /// Creates a size of `width` by `height`.
  #[must_use]
  pub const fn new(width: f32, height: f32) -> Size {
    Size { width, height }
  }
}

/// A position.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
  /// The horizontal coordinate, growing to the right.
  pub x: f32,
  /// The vertical coordinate, growing downward.
  pub y: f32,
}

impl Point {
  /// The origin.
  pub const ZERO: Point = Point::new(0.0, 0.0);

  /// Creates the point at `x`, `y`.
  #[must_use]
  pub const fn new(x: f32, y: f32) -> Point {
    Point { x, y }
  }
}

/// A rectangle: a position and a size.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
  /// The left edge.
  pub x: f32,
  /// The top edge.
  pub y: f32,
  /// The horizontal extent.
  pub width: f32,
  /// The vertical extent.
  pub height: f32,
}

impl Rect {
  /// An empty rectangle at the origin.
  pub const ZERO: Rect = Rect::new(0.0, 0.0, 0.0, 0.0);

  /// Creates the rectangle at `x`, `y` of `width` by `height`.
  #[must_use]
  pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect { x, y, width, height }
  }

  /// The top-left corner.
  #[must_use]
  pub const fn origin(self) -> Point {
    Point::new(self.x, self.y)
  }

  /// The width and height.
  #[must_use]
  pub const fn size(self) -> Size {
    Size::new(self.width, self.height)
  }

  /// Shrinks the rectangle by `thickness` on each side, as padding does to the space for content.
  ///
  /// The width and height stop at zero when the thickness is larger than the rectangle.
  #[must_use]
  pub fn deflate(self, thickness: Thickness) -> Rect {
    Rect::new(
      self.x + thickness.left,
      self.y + thickness.top,
      (self.width - thickness.horizontal()).max(0.0),
      (self.height - thickness.vertical()).max(0.0),
    )
  }

  /// This rectangle with its edges on the device-pixel grid of a display at `scale` device pixels
  /// per logical pixel.
  ///
  /// Each edge is rounded on its own and the size is what lies between them, so rectangles that
  /// share an edge keep sharing it: three columns of `33.33` become `33`, `34` and `33` wide,
  /// without a gap.
  #[must_use]
  pub fn snap(self, scale: f32) -> Rect {
    let edge = |value: f32| (value * scale).round() / scale;
    let (left, top) = (edge(self.x), edge(self.y));
    Rect::new(left, top, edge(self.x + self.width) - left, edge(self.y + self.height) - top)
  }
}

/// The width of space on each side of a box, such as a margin, a padding or a border.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Thickness {
  /// The space on the left.
  pub left: f32,
  /// The space at the top.
  pub top: f32,
  /// The space on the right.
  pub right: f32,
  /// The space at the bottom.
  pub bottom: f32,
}

impl Thickness {
  /// No space on any side.
  pub const ZERO: Thickness = Thickness::uniform(0.0);

  /// Creates a thickness with each side given separately.
  #[must_use]
  pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Thickness {
    Thickness {
      left,
      top,
      right,
      bottom,
    }
  }

  /// Creates a thickness with the same width on every side.
  #[must_use]
  pub const fn uniform(width: f32) -> Thickness {
    Thickness::new(width, width, width, width)
  }

  /// Creates a thickness with `horizontal` on the left and right and `vertical` at the top and
  /// bottom.
  #[must_use]
  pub const fn symmetric(horizontal: f32, vertical: f32) -> Thickness {
    Thickness::new(horizontal, vertical, horizontal, vertical)
  }

  /// The total width it adds across: left plus right.
  #[must_use]
  pub const fn horizontal(self) -> f32 {
    self.left + self.right
  }

  /// The total height it adds: top plus bottom.
  #[must_use]
  pub const fn vertical(self) -> f32 {
    self.top + self.bottom
  }
}

impl Add<Thickness> for Size {
  type Output = Size;

  /// Grows the size by the thickness on each side, as a margin does to a desired size.
  fn add(self, thickness: Thickness) -> Size {
    Size::new(self.width + thickness.horizontal(), self.height + thickness.vertical())
  }
}

impl Sub<Thickness> for Size {
  type Output = Size;

  /// Shrinks the size by the thickness on each side, stopping at zero, as padding does to the space
  /// left for content.
  fn sub(self, thickness: Thickness) -> Size {
    Size::new(
      (self.width - thickness.horizontal()).max(0.0),
      (self.height - thickness.vertical()).max(0.0),
    )
  }
}

/// The radius of each corner of a box, clockwise from the top-left.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CornerRadius {
  /// The top-left radius.
  pub top_left: f32,
  /// The top-right radius.
  pub top_right: f32,
  /// The bottom-right radius.
  pub bottom_right: f32,
  /// The bottom-left radius.
  pub bottom_left: f32,
}

impl CornerRadius {
  /// Square corners.
  pub const ZERO: CornerRadius = CornerRadius::uniform(0.0);

  /// Creates a corner radius with each corner given separately, clockwise from the top-left.
  #[must_use]
  pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> CornerRadius {
    CornerRadius {
      top_left,
      top_right,
      bottom_right,
      bottom_left,
    }
  }

  /// Creates a corner radius with the same radius on every corner.
  #[must_use]
  pub const fn uniform(radius: f32) -> CornerRadius {
    CornerRadius::new(radius, radius, radius, radius)
  }
}

/// A rectangle with rounded corners.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RoundedRect {
  /// The rectangle the corners are cut from.
  pub rect: Rect,
  /// The radius of each corner.
  pub radius: CornerRadius,
}

impl RoundedRect {
  /// Creates `rect` with corners of `radius`.
  #[must_use]
  pub const fn new(rect: Rect, radius: CornerRadius) -> RoundedRect {
    RoundedRect { rect, radius }
  }

  /// The same shape with radii that can be drawn: a negative, NaN or infinite radius is `0`, and
  /// when two radii on one side add up to more than the side, all four shrink by the same factor,
  /// as in CSS.
  #[must_use]
  pub fn fitted(self) -> RoundedRect {
    let drawable = |radius: f32| {
      if radius > 0.0 && radius.is_finite() {
        f64::from(radius)
      } else {
        0.0
      }
    };
    let CornerRadius {
      top_left,
      top_right,
      bottom_right,
      bottom_left,
    } = self.radius;
    let [top_left, top_right, bottom_right, bottom_left] = [top_left, top_right, bottom_right, bottom_left].map(drawable);
    let (width, height) = (f64::from(self.rect.width.max(0.0)), f64::from(self.rect.height.max(0.0)));
    let scale = [
      (width, top_left + top_right),
      (width, bottom_left + bottom_right),
      (height, top_left + bottom_left),
      (height, top_right + bottom_right),
    ]
    .into_iter()
    .filter(|&(side, radii)| radii > side)
    .map(|(side, radii)| side / radii)
    .fold(1.0, f64::min);
    let [top_left, top_right, bottom_right, bottom_left] =
      [top_left, top_right, bottom_right, bottom_left].map(|radius| (radius * scale) as f32);
    RoundedRect::new(self.rect, CornerRadius::new(top_left, top_right, bottom_right, bottom_left))
  }
}

impl From<Rect> for RoundedRect {
  /// The rectangle with square corners.
  fn from(rect: Rect) -> RoundedRect {
    RoundedRect::new(rect, CornerRadius::ZERO)
  }
}

impl Display for Size {
  /// Writes `width,height`.
  fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
    write!(formatter, "{},{}", self.width, self.height)
  }
}

impl Display for Point {
  /// Writes `x,y`.
  fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
    write!(formatter, "{},{}", self.x, self.y)
  }
}

impl Display for Rect {
  /// Writes `x,y,width,height`.
  fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
    write!(formatter, "{},{},{},{}", self.x, self.y, self.width, self.height)
  }
}

impl Display for Thickness {
  /// Writes `left,top,right,bottom`.
  fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
    write!(formatter, "{},{},{},{}", self.left, self.top, self.right, self.bottom)
  }
}

impl Display for CornerRadius {
  /// Writes `top_left,top_right,bottom_right,bottom_left`.
  fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
    write!(
      formatter,
      "{},{},{},{}",
      self.top_left, self.top_right, self.bottom_right, self.bottom_left
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  // Snapped edges land on whole device pixels at any scale, and neighbors still share their edge.
  #[test]
  fn snap_rounds_edges_to_device_pixels() {
    for scale in [1.0_f32, 1.25, 2.0] {
      let columns: Vec<Rect> = (0..3)
        .map(|index| Rect::new(index as f32 * 100.0 / 3.0, 0.3, 100.0 / 3.0, 10.4).snap(scale))
        .collect();

      for rect in &columns {
        for edge in [rect.x, rect.y, rect.x + rect.width, rect.y + rect.height] {
          let device = edge * scale;
          assert!((device - device.round()).abs() < 1e-3, "{rect} at scale {scale}");
        }
      }
      for pair in columns.windows(2) {
        assert_eq!(pair[0].x + pair[0].width, pair[1].x, "no gap at scale {scale}");
      }
    }
    let widths: Vec<f32> = (0..3)
      .map(|index| Rect::new(index as f32 * 100.0 / 3.0, 0.0, 100.0 / 3.0, 1.0).snap(1.0).width)
      .collect();
    assert_eq!(widths, [33.0, 34.0, 33.0]);
  }

  // Each type's zero constant is its default.
  #[test]
  fn zero_constants_are_the_defaults() {
    assert_eq!(Thickness::ZERO, Thickness::default());
    assert_eq!(CornerRadius::ZERO, CornerRadius::default());
    assert_eq!(Size::ZERO, Size::default());
    assert_eq!(Point::ZERO, Point::default());
    assert_eq!(Rect::ZERO, Rect::default());
  }

  // The shorthand constructors spread their values over the sides and corners.
  #[test]
  fn shorthand_constructors() {
    assert_eq!(Thickness::uniform(4.0), Thickness::new(4.0, 4.0, 4.0, 4.0));
    assert_eq!(Thickness::symmetric(4.0, 8.0), Thickness::new(4.0, 8.0, 4.0, 8.0));
    assert_eq!(CornerRadius::uniform(2.0), CornerRadius::new(2.0, 2.0, 2.0, 2.0));
  }

  // Display writes every component, comma-separated, in field order.
  #[test]
  fn display_writes_the_canonical_form() {
    assert_eq!(Thickness::symmetric(4.0, 8.5).to_string(), "4,8.5,4,8.5");
    assert_eq!(CornerRadius::new(1.0, 2.0, 3.0, 4.0).to_string(), "1,2,3,4");
    assert_eq!(Rect::new(1.0, 2.0, 30.0, 40.0).to_string(), "1,2,30,40");
    assert_eq!(Size::new(30.0, 40.0).to_string(), "30,40");
    assert_eq!(Point::new(-1.5, 2.0).to_string(), "-1.5,2");
  }

  // A rectangle splits into its origin and size.
  #[test]
  fn rect_origin_and_size() {
    let rect = Rect::new(1.0, 2.0, 30.0, 40.0);

    assert_eq!(rect.origin(), Point::new(1.0, 2.0));
    assert_eq!(rect.size(), Size::new(30.0, 40.0));
  }

  // Radii that two corners of one side can't share shrink together, by the factor the tightest
  // side needs, as in CSS, however large they are; radii that fit are kept, and negative, NaN or
  // infinite ones are 0.
  #[test]
  fn rounded_rect_fitted_scales_overlapping_radii() {
    let rect = Rect::new(0.0, 0.0, 10.0, 40.0);
    let fitted = |radius| RoundedRect::new(rect, radius).fitted().radius;

    assert_eq!(fitted(CornerRadius::uniform(5.0)), CornerRadius::uniform(5.0));
    assert_eq!(
      fitted(CornerRadius::new(10.0, 10.0, 0.0, 0.0)),
      CornerRadius::new(5.0, 5.0, 0.0, 0.0)
    );
    assert_eq!(
      fitted(CornerRadius::new(4.0, 0.0, 20.0, 0.0)),
      CornerRadius::new(2.0, 0.0, 10.0, 0.0)
    );
    assert_eq!(
      fitted(CornerRadius::new(-1.0, f32::NAN, f32::INFINITY, 3.0)),
      CornerRadius::new(0.0, 0.0, 0.0, 3.0)
    );
    assert_eq!(RoundedRect::new(rect, CornerRadius::uniform(5.0)).fitted().rect, rect);
    assert_eq!(
      fitted(CornerRadius::new(f32::MAX, f32::MAX, 0.0, 0.0)),
      CornerRadius::new(5.0, 5.0, 0.0, 0.0)
    );
  }

  // A rectangle is a rounded rectangle with square corners.
  #[test]
  fn rect_converts_into_rounded_rect() {
    let rect = Rect::new(1.0, 2.0, 3.0, 4.0);

    assert_eq!(RoundedRect::from(rect), RoundedRect::new(rect, CornerRadius::ZERO));
  }

  // Adding a thickness grows a size by both sides; subtracting shrinks it and stops at zero.
  #[test]
  fn size_arithmetic_with_thickness() {
    let padding = Thickness::new(1.0, 2.0, 3.0, 4.0);

    assert_eq!(Size::new(10.0, 10.0) + padding, Size::new(14.0, 16.0));
    assert_eq!(Size::new(10.0, 10.0) - padding, Size::new(6.0, 4.0));
    assert_eq!(Size::new(3.0, 3.0) - padding, Size::ZERO);
    assert_eq!(Size::INFINITY - padding, Size::INFINITY);
  }

  // Deflating moves the origin in by the left and top widths and shrinks, stopping at zero.
  #[test]
  fn rect_deflate() {
    let padding = Thickness::new(1.0, 2.0, 3.0, 4.0);

    assert_eq!(
      Rect::new(10.0, 10.0, 20.0, 20.0).deflate(padding),
      Rect::new(11.0, 12.0, 16.0, 14.0)
    );
    assert_eq!(Rect::new(0.0, 0.0, 2.0, 2.0).deflate(padding), Rect::new(1.0, 2.0, 0.0, 0.0));
  }
}
