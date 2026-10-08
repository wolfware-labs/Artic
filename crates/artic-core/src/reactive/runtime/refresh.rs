use crate::reactive::runtime::{EffectId, MemoId, ObservableId, ObserverId, RUNTIME, Runtime};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::thread;

impl Runtime {
  pub fn refresh_memo(memo_id: MemoId) {
    let (observer_id, observables, is_computing) = RUNTIME.with(|runtime| {
      (
        runtime.memos.observer(memo_id),
        runtime.memos.observables(memo_id),
        runtime.memos.is_computing(memo_id),
      )
    });
    if is_computing {
      panic!("memo read while it is being computed; a memo can't depend on itself");
    }
    if !Self::refresh_observables(observer_id, &observables) {
      return;
    }
    let compute = RUNTIME.with(|runtime| {
      runtime.memos.set_observables(memo_id, Vec::new());
      runtime.memos.set_computing(memo_id, true);
      runtime.memos.compute(memo_id)
    });
    let compute_result = Self::rerun(observer_id, &observables, &*compute);
    RUNTIME.with(|runtime| {
      runtime.memos.set_computing(memo_id, false);
      match compute_result {
        Ok(observables) => runtime.memos.set_observables(memo_id, observables),
        Err(payload) => {
          runtime.observers.mark_as_dirty(&[observer_id]);
          resume_unwind(payload);
        }
      }
    })
  }

  pub fn refresh_effect(effect_id: EffectId) {
    if !RUNTIME.with(|runtime| runtime.effects.contains(effect_id)) {
      return;
    }
    let (observer_id, observables, is_running) = RUNTIME.with(|runtime| {
      (
        runtime.effects.observer(effect_id),
        runtime.effects.observables(effect_id),
        runtime.effects.is_running(effect_id),
      )
    });
    if is_running || !Self::refresh_observables(observer_id, &observables) {
      return;
    }
    let run = RUNTIME.with(|runtime| {
      runtime.effects.set_observables(effect_id, Vec::new());
      runtime.effects.set_running(effect_id, true);
      runtime.effects.run(effect_id)
    });
    let run_result = Self::rerun(observer_id, &observables, || (run.borrow_mut())());
    if !RUNTIME.with(|runtime| runtime.effects.contains(effect_id)) {
      match run_result {
        Ok(observables) => {
          for observable in observables {
            observable.unsubscribe(observer_id);
          }
        }
        Err(payload) => resume_unwind(payload),
      }
      return;
    }
    RUNTIME.with(|runtime| {
      runtime.effects.set_running(effect_id, false);
      match run_result {
        Ok(observables) => runtime.effects.set_observables(effect_id, observables),
        Err(payload) => {
          runtime.observers.mark_as_dirty(&[observer_id]);
          resume_unwind(payload);
        }
      }
    })
  }

  fn refresh_observables(observer_id: ObserverId, observables: &[ObservableId]) -> bool {
    for observable in observables {
      if Self::is_observer_dirty(observer_id) {
        break;
      }
      observable.refresh();
    }
    Self::is_observer_dirty(observer_id)
  }

  fn rerun(observer_id: ObserverId, observables: &[ObservableId], action: impl FnOnce()) -> thread::Result<Vec<ObservableId>> {
    for observable in observables {
      observable.unsubscribe(observer_id);
    }
    catch_unwind(AssertUnwindSafe(|| Self::run_tracked(observer_id, action)))
  }
}
