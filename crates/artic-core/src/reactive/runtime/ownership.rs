use crate::reactive::runtime::{EffectId, MemoId, RUNTIME, ReactiveNodeId, Runtime, ScopeId, SignalId};
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

  pub fn dispose_scope(scope_id: ScopeId) {
    if !RUNTIME.with(|runtime| runtime.scopes.contains(scope_id)) {
      return;
    }
    Self::clear_scope(scope_id);
    RUNTIME.with(|runtime| {
      if let Some(parent) = runtime.scopes.parent(scope_id) {
        runtime.scopes.remove_child(parent, scope_id);
      }
      runtime.scopes.free(scope_id);
    });
  }

  pub(super) fn clear_scope(scope_id: ScopeId) {
    let (children, nodes) =
      RUNTIME.with(|runtime| (runtime.scopes.take_children(scope_id), runtime.scopes.take_nodes(scope_id)));
    for child in children {
      Self::dispose_scope(child);
    }
    for node in &nodes {
      if let ReactiveNodeId::Effect(effect_id) = node {
        Self::dispose_effect(*effect_id);
      }
    }
    for node in &nodes {
      if let ReactiveNodeId::Memo(memo_id) = node {
        Self::dispose_memo(*memo_id);
      }
    }
    for node in &nodes {
      if let ReactiveNodeId::Signal(signal_id) = node {
        Self::dispose_signal(*signal_id);
      }
    }
  }

  fn dispose_effect(effect_id: EffectId) {
    let (observer_id, scope_id, observables) = RUNTIME.with(|runtime| {
      (
        runtime.effects.observer(effect_id),
        runtime.effects.scope(effect_id),
        runtime.effects.observables(effect_id),
      )
    });
    Self::dispose_scope(scope_id);
    for observable in observables.iter() {
      observable.unsubscribe(observer_id);
    }
    RUNTIME.with(|runtime| {
      runtime.effects.free(effect_id);
      if !runtime.scheduler.is_flushing() {
        runtime.effects.remove_from_creation_order(effect_id);
      }
      runtime.observers.free(observer_id);
    });
  }

  fn dispose_memo(memo_id: MemoId) {
    let (observer_id, scope_id, value_id, observables) = RUNTIME.with(|runtime| {
      (
        runtime.memos.observer(memo_id),
        runtime.memos.scope(memo_id),
        runtime.memos.value(memo_id),
        runtime.memos.observables(memo_id),
      )
    });
    Self::dispose_scope(scope_id);
    for observable in observables.iter() {
      observable.unsubscribe(observer_id);
    }
    RUNTIME.with(|runtime| {
      runtime.memos.free(memo_id);
      runtime.observers.free(observer_id);
      runtime.signals.free(value_id);
    });
  }

  fn dispose_signal(signal_id: SignalId) {
    RUNTIME.with(|runtime| runtime.signals.free(signal_id));
  }

  pub(super) fn register_node(&self, node: ReactiveNodeId) {
    if let Some(scope_id) = self.scopes.active() {
      self.scopes.add_node(scope_id, node);
    }
  }
}
