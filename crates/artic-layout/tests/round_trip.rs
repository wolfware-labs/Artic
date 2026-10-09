// Property-based round trips: formatting a geometry value and parsing the text gives the same
// value back.

use artic_layout::geometry::{CornerRadius, Rect, Thickness};
use proptest::prelude::*;

/// Finite `f32` values of any sign, including `-0.0`.
fn finite() -> impl Strategy<Value = f32> {
  any::<f32>().prop_filter("finite", |value| value.is_finite())
}

/// Finite `f32` values that aren't negative.
fn not_negative() -> impl Strategy<Value = f32> {
  finite().prop_map(f32::abs)
}

proptest! {
  // Four finite values written as "l,t,r,b" parse back to the same Thickness.
  #[test]
  fn thickness_round_trips(left in finite(), top in finite(), right in finite(), bottom in finite()) {
    let thickness = Thickness::new(left, top, right, bottom);

    let text = format!("{left},{top},{right},{bottom}");

    prop_assert_eq!(text.parse::<Thickness>(), Ok(thickness));
    prop_assert_eq!(thickness.to_string().parse::<Thickness>(), Ok(thickness));
  }

  // A CornerRadius and a Rect parse back from their Display text.
  #[test]
  fn corner_radius_and_rect_round_trip(
    radii in prop::array::uniform4(not_negative()),
    (x, y) in (finite(), finite()),
    (width, height) in (not_negative(), not_negative()),
  ) {
    let corners = CornerRadius::new(radii[0], radii[1], radii[2], radii[3]);
    let rect = Rect::new(x, y, width, height);

    prop_assert_eq!(corners.to_string().parse::<CornerRadius>(), Ok(corners));
    prop_assert_eq!(rect.to_string().parse::<Rect>(), Ok(rect));
  }
}
