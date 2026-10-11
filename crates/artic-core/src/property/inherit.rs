//! Functions a [`Combine`](super::Inheritance::Combine) key joins its own value and its source's
//! with.

/// Enabled only while the source is: `own && inherited`. A disabled ancestor disables every
/// descendant, whatever the descendant sets itself.
#[must_use]
pub fn and(own: &bool, inherited: &bool) -> bool {
  *own && *inherited
}
