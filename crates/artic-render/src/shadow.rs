use crate::{Color, Shape};
use artic_layout::geometry::{CornerRadius, Point, Rect, RoundedRect};

/// A blurred copy of a rounded rectangle, drawn behind it: CSS's outer `box-shadow`.
///
/// The rectangle is grown by `spread`, moved by `offset` and blurred by `blur`, in `color`. It's
/// built with [`new`](BoxShadow::new) and the `with_*` methods, so fields added later break no
/// caller.
///
/// # Examples
///
/// ```
/// use artic_layout::geometry::Point;
/// use artic_render::{BoxShadow, Color};
///
/// let shadow = BoxShadow::new(Point::new(0.0, 2.0), 8.0, Color::rgba(0, 0, 0, 64)).with_spread(2.0);
/// assert_eq!(shadow.spread, 2.0);
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BoxShadow {
  /// How far the shadow is moved from the rectangle that casts it.
  pub offset: Point,
  /// How far the edges fade: twice the standard deviation of the gaussian blur, as in CSS.
  pub blur: f32,
  /// How far the rectangle grows on each side before it's blurred. A negative spread shrinks it.
  pub spread: f32,
  /// The shadow's color.
  pub color: Color,
}

impl BoxShadow {
  /// A shadow moved by `offset` and blurred by `blur`, in `color`, with no spread.
  #[must_use]
  pub fn new(offset: Point, blur: f32, color: Color) -> BoxShadow {
    BoxShadow {
      offset,
      blur,
      spread: 0.0,
      color,
    }
  }

  /// The same shadow with `spread`.
  #[must_use]
  pub fn with_spread(self, spread: f32) -> BoxShadow {
    BoxShadow { spread, ..self }
  }

  /// The shadow as it's drawn, or `None` when it draws nothing: an offset with a NaN or infinite
  /// coordinate. A blur below `0`, NaN or infinite draws as `0`, a sharp shadow, and a NaN or
  /// infinite spread as `0`.
  #[must_use]
  pub fn drawable(&self) -> Option<BoxShadow> {
    let finite = |value: f32| if value.is_finite() { value } else { 0.0 };
    (self.offset.x.is_finite() && self.offset.y.is_finite()).then(|| BoxShadow {
      blur: finite(self.blur).max(0.0),
      spread: finite(self.spread),
      ..self.clone()
    })
  }

  /// The rounded rectangle this shadow blurs when `rect` casts it, or `None` when it draws
  /// nothing.
  ///
  /// `rect` is made [drawable](Shape::drawable), grown by the spread on each side and moved by
  /// the offset. Each radius above `0` grows by the spread, and the radii are
  /// [fitted](RoundedRect::fitted) again. A negative spread shrinks the rectangle, down to
  /// nothing. The shadow's own values are taken as they are, so call it on a
  /// [drawable](BoxShadow::drawable) shadow.
  #[must_use]
  pub fn shape(&self, rect: RoundedRect) -> Option<RoundedRect> {
    let Some(Shape::RoundedRect(caster)) = Shape::from(rect).drawable() else {
      return None;
    };
    let (spread, offset) = (self.spread, self.offset);
    let Rect { x, y, width, height } = caster.rect;
    let grown = Rect::new(
      x - spread + offset.x,
      y - spread + offset.y,
      width + 2.0 * spread,
      height + 2.0 * spread,
    );
    let grow = |radius: f32| if radius > 0.0 { radius + spread } else { 0.0 };
    let CornerRadius {
      top_left,
      top_right,
      bottom_right,
      bottom_left,
    } = caster.radius;
    let radius = CornerRadius::new(grow(top_left), grow(top_right), grow(bottom_right), grow(bottom_left));
    match Shape::from(RoundedRect::new(grown, radius)).drawable() {
      Some(Shape::RoundedRect(shadow)) if grown.width > 0.0 && grown.height > 0.0 => Some(shadow),
      _ => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  const BLACK: Color = Color::rgb(0, 0, 0);

  // `new` has no spread, and `with_spread` sets only the spread.
  #[test]
  fn box_shadow_new_has_no_spread() {
    let shadow = BoxShadow::new(Point::new(1.0, 2.0), 3.0, BLACK);

    assert_eq!(
      (shadow.offset, shadow.blur, shadow.spread, shadow.color),
      (Point::new(1.0, 2.0), 3.0, 0.0, BLACK)
    );
    assert_eq!(shadow.clone().with_spread(4.0), BoxShadow { spread: 4.0, ..shadow });
  }

  // An offset that isn't finite draws nothing. A blur below 0, NaN or infinite draws as 0, and a
  // NaN or infinite spread as 0; a negative spread stays, to shrink the rectangle.
  #[test]
  fn box_shadow_values_are_made_drawable() {
    let drawn = |blur, spread| {
      BoxShadow::new(Point::ZERO, blur, BLACK)
        .with_spread(spread)
        .drawable()
        .map(|shadow| (shadow.blur, shadow.spread))
    };

    for offset in [
      Point::new(f32::NAN, 0.0),
      Point::new(0.0, f32::INFINITY),
      Point::new(f32::NEG_INFINITY, 0.0),
    ] {
      assert_eq!(BoxShadow::new(offset, 1.0, BLACK).drawable(), None, "offset {offset}");
    }
    assert_eq!(
      [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 4.0].map(|blur| drawn(blur, 0.0)),
      [
        Some((0.0, 0.0)),
        Some((0.0, 0.0)),
        Some((0.0, 0.0)),
        Some((0.0, 0.0)),
        Some((4.0, 0.0))
      ]
    );
    assert_eq!(
      [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -2.0].map(|spread| drawn(1.0, spread)),
      [Some((1.0, 0.0)), Some((1.0, 0.0)), Some((1.0, 0.0)), Some((1.0, -2.0))]
    );
  }

  // The shadow's rectangle is the caster's, grown by the spread on each side and moved by the
  // offset. Rounded corners grow by the spread, square ones stay square, and a corner shrunk past
  // 0 is square. A negative spread shrinks the rectangle, and nothing is left once it has no width
  // or height, or once a coordinate isn't finite.
  #[test]
  fn box_shadow_shape_grows_and_moves() {
    let card = RoundedRect::new(Rect::new(10.0, 20.0, 100.0, 50.0), CornerRadius::new(2.0, 0.0, 8.0, 0.0));
    let spread = |spread| BoxShadow::new(Point::ZERO, 0.0, BLACK).with_spread(spread);
    let crowded = RoundedRect::new(Rect::new(0.0, 0.0, 10.0, 10.0), CornerRadius::new(5.0, 5.0, 0.0, 0.0));

    assert_eq!(
      BoxShadow::new(Point::new(3.0, 5.0), 8.0, BLACK).with_spread(2.0).shape(card),
      Some(RoundedRect::new(
        Rect::new(11.0, 23.0, 104.0, 54.0),
        CornerRadius::new(4.0, 0.0, 10.0, 0.0)
      ))
    );
    assert_eq!(
      spread(-3.0).shape(card),
      Some(RoundedRect::new(
        Rect::new(13.0, 23.0, 94.0, 44.0),
        CornerRadius::new(0.0, 0.0, 5.0, 0.0)
      ))
    );
    assert_eq!(
      spread(2.0).shape(crowded),
      Some(RoundedRect::new(
        Rect::new(-2.0, -2.0, 14.0, 14.0),
        CornerRadius::new(7.0, 7.0, 0.0, 0.0)
      )),
      "radii that meet keep meeting"
    );
    assert_eq!(spread(-25.0).shape(card), None, "no height left");
    assert_eq!(spread(f32::MAX).shape(card), None, "overflows");
    assert_eq!(
      spread(0.0).shape(Rect::new(0.0, 0.0, f32::NAN, 1.0).into()),
      None,
      "the caster draws nothing"
    );
  }
}
