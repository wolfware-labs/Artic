use crate::reactive::runtime::{Runtime, ScopeId};
use std::marker::PhantomData;

/// An owner of reactive nodes, and the unit they are disposed in.
///
/// Scopes form a tree: a root scope has no parent, and [`child`](Scope::child) creates a scope
/// inside another one. [`run`](Scope::run) makes a scope the active one while a closure runs, and
/// every signal, memo and effect created during that time belongs to it.
/// [`dispose`](Scope::dispose) frees them all at once.
///
/// `Scope` is a `Copy` handle: copies refer to the same scope. It is neither `Send` nor `Sync`.
///
/// # Examples
///
/// ```
/// use artic_core::reactive::{Scope, Signal, live_counts};
///
/// let before = live_counts();
/// let view = Scope::new_root();
/// let email = view.run(|| Signal::new(String::new()));
/// email.set(String::from("ada@example.com"));
///
/// view.dispose();
/// assert_eq!(live_counts(), before);
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

  /// Frees this scope, its child scopes and every signal, memo and effect created while it was
  /// active.
  ///
  /// Child scopes go first, then effects, memos and signals. A disposed effect never runs again,
  /// and signals and memos that outlive it stop listing it as a reader.
  pub fn dispose(&self) {
    Runtime::dispose_scope(self.0);
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
