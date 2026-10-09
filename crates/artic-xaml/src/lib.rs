//! XAML parser producing the artic infoset, with spanned diagnostics.
//!
//! [`parse`] reads one file of a [`SourceMap`](diagnostics::SourceMap) into an
//! [`XamlDocument`](infoset::XamlDocument). XML is read with the `xmlparser` tokenizer, which keeps
//! byte positions for every token, so each node of the infoset points at its source text and every
//! error is reported with a span.
//!
//! ```
//! use artic_xaml::diagnostics::SourceMap;
//! use artic_xaml::parse;
//!
//! let mut sources = SourceMap::new();
//! let file = sources.add("view.xaml", "<Button Width=\"100\"></Panel>");
//! let errors = parse(&sources, file).expect_err("mismatched end tag");
//! assert!(errors.render(&sources).contains("expected `</Button>`, found `</Panel>`"));
//! ```

pub mod diagnostics;
pub mod infoset;
mod parse;

pub use parse::parse;
