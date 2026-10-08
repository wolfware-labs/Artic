use crate::reactive::runtime::{Runtime, ScopeId};
use std::marker::PhantomData;

/// An owner of reactive nodes, and the unit they are disposed in.
///
/// Scopes form a tree: a root scope has no parent, and [`child`](Scope::child) creates a scope
/// inside another one.
///
/// `Scope` is a `Copy` handle: copies refer to the same scope. It is neither `Send` nor `Sync`.
///
/// # Examples
///
/// ```
/// use artic_core::reactive::Scope;
///
/// let window = Scope::new_root();
/// let view = window.child();
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Scope(ScopeId, PhantomData<*const ()>);

impl Scope {
  /// Creates a scope with no parent.
  pub fn new_root() -> Scope {
    Scope(Runtime::create_scope(None), PhantomData)
  }

  /// Creates a scope owned by this one.
  pub fn child(&self) -> Scope {
    Scope(Runtime::create_scope(Some(self.0)), PhantomData)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use static_assertions::{assert_impl_all, assert_not_impl_any};

  assert_not_impl_any!(Scope: Send, Sync);
  assert_impl_all!(Scope: Copy);
}
