use crate::reactive::runtime::{EffectId, MemoId, Node, RUNTIME, Runtime};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::thread;

impl Runtime {
  pub fn refresh_memo(memo_id: MemoId) {
    if !RUNTIME.with(|runtime| runtime.memos.contains(memo_id)) {
      return;
    }
    let node = Node::Memo(memo_id);
    if RUNTIME.with(|runtime| runtime.memos.is_computing(memo_id)) {
      panic!("memo read while it is being computed; a memo can't depend on itself");
    }
    if !Self::refresh_sources(node) {
      return;
    }
    let (scope_id, compute) = RUNTIME.with(|runtime| {
      runtime.memos.set_computing(memo_id, true);
      (runtime.scopes.scope_of(node), runtime.memos.compute(memo_id))
    });
    Self::clear_scope(scope_id);
    let compute_result = Self::rerun(node, || Self::run_in_scope(scope_id, || compute(memo_id)));
    if !RUNTIME.with(|runtime| runtime.memos.contains(memo_id)) {
      Self::discard_run(node, compute_result);
      return;
    }
    RUNTIME.with(|runtime| {
      runtime.memos.set_computing(memo_id, false);
      if let Err(payload) = compute_result {
        runtime.set_dirty(node, true);
        resume_unwind(payload);
      }
    })
  }

  pub fn refresh_effect(effect_id: EffectId) {
    if !RUNTIME.with(|runtime| runtime.effects.contains(effect_id)) {
      return;
    }
    let node = Node::Effect(effect_id);
    if RUNTIME.with(|runtime| runtime.effects.is_running(effect_id)) || !Self::refresh_sources(node) {
      return;
    }
    let (scope_id, run) = RUNTIME.with(|runtime| {
      runtime.effects.set_running(effect_id, true);
      (runtime.scopes.scope_of(node), runtime.effects.run(effect_id))
    });
    Self::clear_scope(scope_id);
    let run_result = Self::rerun(node, || Self::run_in_scope(scope_id, || (run.borrow_mut())()));
    if !RUNTIME.with(|runtime| runtime.effects.contains(effect_id)) {
      Self::discard_run(node, run_result);
      return;
    }
    RUNTIME.with(|runtime| {
      runtime.effects.set_running(effect_id, false);
      if let Err(payload) = run_result {
        runtime.set_dirty(node, true);
        resume_unwind(payload);
      }
    })
  }

  fn refresh_sources(node: Node) -> bool {
    let mut index = 0;
    while !Self::is_dirty(node) {
      let Some(source) = RUNTIME.with(|runtime| runtime.graph.source_at(node, index)) else {
        break;
      };
      source.refresh();
      index += 1;
    }
    Self::is_dirty(node)
  }

  fn discard_run(node: Node, run_result: thread::Result<()>) {
    RUNTIME.with(|runtime| runtime.graph.clear_sources(node));
    if let Err(payload) = run_result {
      resume_unwind(payload);
    }
  }

  fn rerun(node: Node, action: impl FnOnce()) -> thread::Result<()> {
    RUNTIME.with(|runtime| runtime.graph.clear_sources(node));
    catch_unwind(AssertUnwindSafe(|| Self::run_tracked(node, action)))
  }
}
