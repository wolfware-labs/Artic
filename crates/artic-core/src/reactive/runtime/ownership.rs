use crate::reactive::runtime::{RUNTIME, Runtime, ScopeId};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

impl Runtime {
  pub fn create_scope(parent: Option<ScopeId>) -> ScopeId {
    RUNTIME.with(|runtime| runtime.scopes.alloc(parent))
  }

  pub fn run_in_scope<TResult>(scope_id: ScopeId, action: impl FnOnce() -> TResult) -> TResult {
    RUNTIME.with(|runtime| runtime.scopes.push_active(scope_id));
    let action_result = catch_unwind(AssertUnwindSafe(action));
    RUNTIME.with(|runtime| runtime.scopes.pop_active());
    match action_result {
      Ok(result) => result,
      Err(payload) => resume_unwind(payload),
    }
  }

  #[cfg(test)]
  pub fn active_scope() -> Option<ScopeId> {
    RUNTIME.with(|runtime| runtime.scopes.active())
  }
}
