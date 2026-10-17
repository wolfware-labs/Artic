//! The values text is shaped from and into, and the engine that shapes it.
//!
//! A control describes its text with a font family, weight and style, and draws the paragraph an
//! engine shaped from them.

mod font;
mod paragraph;

pub use font::{
  FamilyName, FamilyProblem, FontFamily, FontStyle, FontWeight, GenericFamily, ParseFontFamilyError, ParseFontWeightError,
  WeightProblem,
};
pub use paragraph::{FontData, Glyph, GlyphRun, Line, Paragraph, Synthesis};
