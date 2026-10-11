//! artic's controls, and the base every control embeds.
//!
//! - [`ElementBase`] holds the properties every control shares; a control flattens it into its own
//!   properties.
//! - [`Visibility`], [`Cursor`] and [`Classes`] are the value types of some of them.

mod classes;
mod element_base;
mod visibility;

pub use classes::Classes;
pub use element_base::ElementBase;
pub use visibility::{Cursor, Visibility};
