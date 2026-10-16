//! Layout tests without a window or a GPU.

mod compare;
mod elements;
mod headless;
mod invariants;
pub mod strategy;

pub use elements::{Boxes, Fixed, Stack};
pub use headless::Headless;
pub use invariants::Invariants;

/// How far apart, in logical units, two layout values may be and still match.
pub const TOLERANCE: f32 = 0.001;
