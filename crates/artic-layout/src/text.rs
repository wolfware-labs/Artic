//! The values text is shaped from and into, and the engine that shapes it.
//!
//! A control describes its text with a font family, weight and style, and draws the paragraph an
//! engine shaped from them.

mod font;

pub use font::{FamilyName, FamilyProblem, FontFamily, GenericFamily, ParseFontFamilyError};
