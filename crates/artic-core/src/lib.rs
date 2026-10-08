//! The foundations of artic: reactive values, properties, and the node tree.
//!
//! - [`reactive`]: signals, memos, effects and scopes, the only change-propagation mechanism.
//! - [`property`]: [`Property`](property::Property), a reactive value that several sources write at
//!   once, ranked by precedence, with metadata for invalidation and inheritance.
//! - [`tree`]: [`TreeArena`](tree::TreeArena), which owns every node and links it into a logical and
//!   a visual tree, with per-node dirty flags.
//! - [`Object`]: the trait every node type implements.
//!
//! The `testing` feature adds `reactive::assert_no_reactive_leaks` for leak checks in other crates'
//! tests.

pub(crate) mod arena;
mod object;
pub mod property;
pub mod reactive;
pub mod tree;

pub use object::Object;
