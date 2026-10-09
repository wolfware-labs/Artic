//! XAML parser producing the artic infoset, with spanned diagnostics.

pub mod diagnostics;
pub mod infoset;
mod parse;

pub use parse::parse;
