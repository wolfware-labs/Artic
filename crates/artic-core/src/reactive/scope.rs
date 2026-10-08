use crate::reactive::runtime::{Runtime, ScopeId};
use std::marker::PhantomData;

/// An owner of reactive nodes, and the unit they are disposed in.
///
/// Scopes form a tree: a root scope has no parent, and [`child`](Scope::child) creates a scope
/// inside another one. [`run`](Scope::run) makes a scope the active one while a closure runs.
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
/// let title = view.run(|| String::from("Login"));
/// assert_eq!(title, "Login");
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

  /// Runs `action` with this scope as the active scope and returns its result.
  ///
  /// Calls to `run` nest: inside `action`, another scope's `run` makes that scope active until it
  /// returns. If `action` panics, the previous scope is active again before the panic continues.
  pub fn run<TResult>(&self, action: impl FnOnce() -> TResult) -> TResult {
    Runtime::run_in_scope(self.0, action)
  }
}

#[cfg(test)]
impl Scope {
  pub(crate) fn id(&self) -> ScopeId {
    self.0
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use static_assertions::{assert_impl_all, assert_not_impl_any};

  assert_not_impl_any!(Scope: Send, Sync);
  assert_impl_all!(Scope: Copy);
}
