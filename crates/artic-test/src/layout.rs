//! Layout tests without a window or a GPU.

mod compare;

/// How far apart, in logical units, two layout values may be and still match.
pub const TOLERANCE: f32 = 0.001;
