// Layout geometry keeps finite inputs finite, however large, at any scale.

use artic_layout::geometry::{Rect, Size, Thickness};
use artic_layout::{HorizontalAlignment, Sizing, VerticalAlignment, arrange_with, measure_with};
use proptest::prelude::*;

fn huge() -> impl Strategy<Value = f32> {
  prop_oneof![Just(f32::MAX), Just(f32::MIN), Just(0.0f32), -1.0e38f32..1.0e38f32]
}

fn huge_rect() -> impl Strategy<Value = Rect> {
  (huge(), huge(), 0.0f32..f32::MAX, 0.0f32..f32::MAX).prop_map(|(x, y, width, height)| Rect::new(x, y, width, height))
}

fn finite(rect: Rect) -> bool {
  [rect.x, rect.y, rect.width, rect.height]
    .iter()
    .all(|value| value.is_finite())
}

fn horizontal() -> impl Strategy<Value = HorizontalAlignment> {
  prop::sample::select(vec![
    HorizontalAlignment::Left,
    HorizontalAlignment::Center,
    HorizontalAlignment::Right,
    HorizontalAlignment::Stretch,
  ])
}

fn vertical() -> impl Strategy<Value = VerticalAlignment> {
  prop::sample::select(vec![
    VerticalAlignment::Top,
    VerticalAlignment::Center,
    VerticalAlignment::Bottom,
    VerticalAlignment::Stretch,
  ])
}

proptest! {
  // Snapping, deflating by margins and arranging with any alignment never produce an infinity
  // from finite values.
  #[test]
  fn geometry_stays_finite(
    rect in huge_rect(),
    side in 0.0f32..f32::MAX,
    scale in prop::sample::select(vec![1.0f32, 1.25, 2.0, 3.0]),
    horizontal in horizontal(),
    vertical in vertical(),
  ) {
    let margin = Thickness::uniform(side);
    prop_assert!(finite(rect.snap(scale)), "snap");
    prop_assert!(finite(rect.deflate(margin)), "deflate");
    let sizing = Sizing { margin, horizontal, vertical, ..Sizing::default() };
    let desired = Size::new(rect.width, rect.height);
    prop_assert!(finite(arrange_with(&sizing, rect, desired)), "arrange_with");
  }
}

// An infinite available axis stays infinite through the margin, so the content may take what it
// wants.
#[test]
fn infinite_available_stays_infinite() {
  let sizing = Sizing {
    margin: Thickness::uniform(5.0),
    ..Sizing::default()
  };
  let mut offered = Size::ZERO;

  measure_with(&sizing, Size::new(100.0, f32::INFINITY), |inner| {
    offered = inner;
    Size::ZERO
  });

  assert_eq!(offered, Size::new(90.0, f32::INFINITY));
}

// An edge too large to scale is already a whole number, so snapping leaves it where it is, left
// of the origin, and the rectangle stays finite.
#[test]
fn huge_edges_snap_in_place() {
  let rect = Rect::new(f32::MIN, 0.0, 10.0, 10.0).snap(3.0);

  assert_eq!(rect.x, f32::MIN);
  assert!(rect.width.is_finite());
}
