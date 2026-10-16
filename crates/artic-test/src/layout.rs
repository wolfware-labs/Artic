//! Layout tests without a window or a GPU.

mod compare;
mod elements;
mod headless;

pub use elements::{Boxes, Fixed, Stack};
pub use headless::Headless;

/// How far apart, in logical units, two layout values may be and still match.
pub const TOLERANCE: f32 = 0.001;
