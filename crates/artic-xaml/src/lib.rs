//! XAML parser producing the artic infoset, with spanned diagnostics.
//!
//! [`parse`] reads one file of a [`SourceMap`](diagnostics::SourceMap) into an
//! [`XamlDocument`](infoset::XamlDocument). XML is read with the `xmlparser` tokenizer, which keeps
//! byte positions for every token, so each node of the infoset points at its source text and every
//! error is reported with a span. Namespace prefixes are resolved with XML's scoping rules while
//! parsing, so every type reference in a returned document knows its namespace.
//!
//! ```
//! use artic_xaml::diagnostics::SourceMap;
//! use artic_xaml::parse;
//!
//! let mut sources = SourceMap::new();
//! let file = sources.add("view.xaml", r#"<Button xmlns="https://artic.rs/ui"></Panel>"#);
//! let errors = parse(&sources, file).expect_err("mismatched end tag");
//! assert!(errors.render(&sources).contains("expected `</Button>`, found `</Panel>`"));
//! ```

pub mod diagnostics;
pub mod infoset;
mod parse;

pub use parse::parse;
