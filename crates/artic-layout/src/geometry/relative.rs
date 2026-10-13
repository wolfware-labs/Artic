use crate::geometry::{Point, Rect};

/// Whether a [`RelativePoint`] or [`RelativeScalar`] is a fraction of the box it is resolved
/// against, or logical units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativeUnit {
  /// A fraction of the box: `0.5` is half its width or height. Written as a percentage, `50%`.
  Relative,
  /// Logical units, from the box's top-left corner for a point.
  Absolute,
}

/// A point resolved against the box being filled, such as a gradient's start: `"50%,25%"` or
/// `"10,20"`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelativePoint {
  /// The fractions of the box, or the logical units from its top-left corner.
  pub point: Point,
  /// How `point` is measured.
  pub unit: RelativeUnit,
}

impl RelativePoint {
  /// The point at these fractions of the box: `relative(0.5, 0.5)` is its center.
  #[must_use]
  pub const fn relative(x: f32, y: f32) -> RelativePoint {
    RelativePoint {
      point: Point::new(x, y),
      unit: RelativeUnit::Relative,
    }
  }

  /// The point at these logical units from the box's top-left corner.
  #[must_use]
  pub const fn absolute(x: f32, y: f32) -> RelativePoint {
    RelativePoint {
      point: Point::new(x, y),
      unit: RelativeUnit::Absolute,
    }
  }

  /// Where the point falls, in the coordinates `rect` is in.
  #[must_use]
  pub fn resolve(self, rect: Rect) -> Point {
    let Point { x, y } = self.point;
    match self.unit {
      RelativeUnit::Relative => Point::new(rect.x + x * rect.width, rect.y + y * rect.height),
      RelativeUnit::Absolute => Point::new(rect.x + x, rect.y + y),
    }
  }
}

/// A length resolved against one extent of the box being filled, such as a gradient's radius:
/// `"50%"` or `"10"`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelativeScalar {
  /// The fraction of the extent, or logical units.
  pub value: f32,
  /// How `value` is measured.
  pub unit: RelativeUnit,
}

impl RelativeScalar {
  /// This fraction of the extent: `relative(0.5)` is half of it.
  #[must_use]
  pub const fn relative(value: f32) -> RelativeScalar {
    RelativeScalar {
      value,
      unit: RelativeUnit::Relative,
    }
  }

  /// This many logical units, whatever the extent.
  #[must_use]
  pub const fn absolute(value: f32) -> RelativeScalar {
    RelativeScalar {
      value,
      unit: RelativeUnit::Absolute,
    }
  }

  /// The length in logical units, against `extent`.
  #[must_use]
  pub fn resolve(self, extent: f32) -> f32 {
    match self.unit {
      RelativeUnit::Relative => self.value * extent,
      RelativeUnit::Absolute => self.value,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  // Percentages scale with the box and start at its corner; logical units only start there.
  #[test]
  fn relative_values_resolve_against_a_box() {
    let rect = Rect::new(10.0, 20.0, 200.0, 100.0);

    assert_eq!(RelativePoint::relative(0.5, 0.25).resolve(rect), Point::new(110.0, 45.0));
    assert_eq!(RelativePoint::absolute(5.0, 6.0).resolve(rect), Point::new(15.0, 26.0));
    assert_eq!(RelativeScalar::relative(0.5).resolve(200.0), 100.0);
    assert_eq!(RelativeScalar::absolute(7.0).resolve(200.0), 7.0);
  }
}
