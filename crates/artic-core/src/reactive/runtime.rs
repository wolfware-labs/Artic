mod effect_store;
mod memo_store;
mod observable;
mod observer_store;
mod signal_store;

pub use effect_store::EffectId;
use effect_store::EffectStore;
pub use memo_store::MemoId;
use memo_store::MemoStore;
pub use observable::ObservableId;
pub use observer_store::ObserverId;
use observer_store::ObserverStore;
pub use signal_store::SignalId;
use signal_store::SignalStore;
use std::any::Any;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;
use std::thread;

thread_local! {
  static RUNTIME: Runtime  = const {
    Runtime::new()
  };
}

pub struct Runtime {
  signals: SignalStore,
  observers: ObserverStore,
  memos: MemoStore,
  effects: EffectStore,
  observables: RefCell<Vec<Vec<ObservableId>>>,
}

impl Runtime {
  const fn new() -> Self {
    Self {
      signals: SignalStore::new(),
      observers: ObserverStore::new(),
      memos: MemoStore::new(),
      effects: EffectStore::new(),
      observables: RefCell::new(Vec::new()),
    }
  }

  pub fn alloc_signal_value<TValue>(value: TValue) -> SignalId
  where
    TValue: Any,
  {
    RUNTIME.with(|runtime| runtime.signals.alloc_value(value))
  }

  pub fn get_signal_value<TValue>(signal_id: SignalId) -> TValue
  where
    TValue: Any + Clone,
  {
    Self::with_signal_value::<TValue, TValue>(signal_id, |v| v.clone())
  }

  pub fn with_signal_value<TValue, TResult>(signal_id: SignalId, action: impl FnOnce(&TValue) -> TResult) -> TResult
  where
    TValue: Any,
  {
    RUNTIME.with(|runtime| {
      runtime.attach_current_observer(signal_id);
      Self::with_untracked_signal_value(signal_id, action)
    })
  }

  pub(crate) fn with_untracked_signal_value<TValue, TResult>(
    signal_id: SignalId,
    action: impl FnOnce(&TValue) -> TResult,
  ) -> TResult
  where
    TValue: Any,
  {
    RUNTIME.with(|runtime| runtime.signals.with_signal_value(signal_id, action))
  }

  pub fn set_signal_value<TValue>(signal_id: SignalId, value: TValue)
  where
    TValue: Any,
  {
    Self::with_signal_mut_value(signal_id, |v| *v = value)
  }

  pub fn with_signal_mut_value<TValue>(signal_id: SignalId, action: impl FnOnce(&mut TValue))
  where
    TValue: Any,
  {
    RUNTIME.with(|runtime| {
      let action_result = runtime.signals.with_signal_mut_value(signal_id, action);
      runtime.mark_observers_as_dirty(signal_id);
      if let Err(payload) = action_result {
        resume_unwind(payload);
      }
    })
  }

  #[cfg(test)]
  pub fn create_observer() -> ObserverId {
    RUNTIME.with(|runtime| runtime.observers.create_observer())
  }

  fn push_observer(observer_id: ObserverId) {
    RUNTIME.with(|runtime| {
      runtime.observers.push_observer(observer_id);
      runtime.observables.borrow_mut().push(Vec::new());
    })
  }

  fn pop_observer() -> Vec<ObservableId> {
    RUNTIME.with(|runtime| {
      runtime.observers.pop_observer();
      runtime.observables.borrow_mut().pop().unwrap_or_default()
    })
  }

  pub fn record_read(observable: ObservableId) {
    RUNTIME.with(|runtime| {
      if let Some(observables) = runtime.observables.borrow_mut().last_mut()
        && !observables.contains(&observable)
      {
        observables.push(observable);
      }
    })
  }

  pub fn is_observer_dirty(observer_id: ObserverId) -> bool {
    RUNTIME.with(|runtime| runtime.observers.is_dirty(observer_id))
  }

  pub fn create_memo(value_id: SignalId, compute: Rc<dyn Fn()>) -> MemoId {
    RUNTIME.with(|runtime| {
      let observer_id = runtime.observers.create_observer();
      runtime.observers.mark_as_dirty(&[observer_id]);
      runtime.memos.alloc(observer_id, value_id, compute)
    })
  }

  pub fn memo_value(memo_id: MemoId) -> SignalId {
    RUNTIME.with(|runtime| runtime.memos.value(memo_id))
  }

  pub fn run_tracked(observer_id: ObserverId, action: impl FnOnce()) -> Vec<ObservableId> {
    RUNTIME.with(|runtime| runtime.observers.mark_as_clean(observer_id));
    Self::push_observer(observer_id);
    let action_result = catch_unwind(AssertUnwindSafe(action));
    let observables = Self::pop_observer();
    if let Err(payload) = action_result {
      for observable in observables {
        observable.unsubscribe(observer_id);
      }
      resume_unwind(payload);
    }
    observables
  }

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

  pub fn create_effect(run: Rc<RefCell<dyn FnMut()>>) -> EffectId {
    let effect_id = RUNTIME.with(|runtime| {
      let observer_id = runtime.observers.create_observer();
      runtime.observers.mark_as_dirty(&[observer_id]);
      runtime.effects.alloc(observer_id, run)
    });
    Self::refresh_effect(effect_id);
    effect_id
  }

  pub fn refresh_effect(effect_id: EffectId) {
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

  fn attach_current_observer(&self, signal_id: SignalId) {
    if let Some(observer_id) = self.observers.current_observer() {
      self.signals.add_subscriber(signal_id, observer_id);
    }
  }

  fn mark_observers_as_dirty(&self, signal_id: SignalId) {
    self.signals.with_subscribers(signal_id, |subscribers| {
      self.observers.mark_as_dirty(subscribers);
    });
  }
}
