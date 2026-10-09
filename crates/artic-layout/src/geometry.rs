//! The value types layout measures and positions with, in logical pixels.

use std::fmt::{Display, Formatter, Result};

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
}
