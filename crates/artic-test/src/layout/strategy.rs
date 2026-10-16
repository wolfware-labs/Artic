//! [`proptest`](mod@proptest) strategies for layout inputs.

use artic_layout::geometry::Size;
use proptest::prelude::*;

/// An available size whose axes are each 0, from 1 to 1000, or infinite, picked on their own.
pub fn available() -> impl Strategy<Value = Size> {
  (axis(), axis()).prop_map(|(width, height)| Size::new(width, height))
}

/// A display scale: 1, 1.25, 1.5, 2 or 3.
pub fn scale() -> impl Strategy<Value = f32> {
  prop::sample::select(vec![1.0, 1.25, 1.5, 2.0, 3.0])
}

fn axis() -> impl Strategy<Value = f32> {
  prop_oneof![Just(0.0), 1.0f32..1000.0, Just(f32::INFINITY)]
}
