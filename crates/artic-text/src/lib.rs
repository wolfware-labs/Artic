//! Text shaping and fonts for artic.
//!
//! [`Engine`] implements [`TextEngine`](artic_layout::text::TextEngine) with `parley`, which shapes
//! text and breaks it into lines, and `fontique`, which finds fonts. Layout asks it for paragraphs;
//! renderers draw them without depending on this crate. See the
//! [text design](https://github.com/wolfware-labs/artic/blob/main/docs/design/text.md).

mod engine;
mod faces;
mod lines;
mod paragraph;
mod properties;

pub use engine::Engine;
