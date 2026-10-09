//! Errors and warnings about XAML files, with the position they refer to.
//!
//! Every part of artic that reads XAML (the parser, the compiler, the loader and the language
//! server) reports through these types, so problems look the same wherever they are found.

mod source;

pub use source::{LineCol, SourceFile, SourceId, SourceMap, Span};
