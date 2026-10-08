mod effect_store;
mod memo_store;
mod observable;
mod observer_store;
mod ownership;
mod refresh;
mod scheduling;
mod scope_store;
mod signal_store;
mod tracking;

use crate::reactive::LiveCounts;
pub use effect_store::EffectId;
use effect_store::EffectStore;
pub use memo_store::MemoId;
use memo_store::MemoStore;
pub use observable::ObservableId;
pub use observer_store::ObserverId;
use observer_store::ObserverStore;
use scheduling::Scheduler;
use scope_store::ReactiveNodeId;
pub use scope_store::ScopeId;
use scope_store::ScopeStore;
pub use signal_store::SignalId;
use signal_store::SignalStore;
use std::any::Any;
use std::cell::RefCell;
use std::panic::resume_unwind;
use std::rc::Rc;
use tracking::Tracker;

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
  scopes: ScopeStore,
  tracker: Tracker,
  scheduler: Scheduler,
}

impl Runtime {
  const fn new() -> Self {
    Self {
      signals: SignalStore::new(),
      observers: ObserverStore::new(),
      memos: MemoStore::new(),
      effects: EffectStore::new(),
      scopes: ScopeStore::new(),
      tracker: Tracker::new(),
      scheduler: Scheduler::new(),
    }
  }

  pub fn alloc_signal_value<TValue>(value: TValue) -> SignalId
  where
    TValue: Any,
  {
    RUNTIME.with(|runtime| runtime.signals.alloc_value(value))
  }

  pub fn create_signal<TValue>(value: TValue) -> SignalId
  where
    TValue: Any,
  {
    RUNTIME.with(|runtime| {
      let signal_id = runtime.signals.alloc_value(value);
      runtime.register_node(ReactiveNodeId::Signal(signal_id));
      signal_id
    })
  }

  pub fn contains_signal(signal_id: SignalId) -> bool {
    RUNTIME.with(|runtime| runtime.signals.contains(signal_id))
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
      runtime.subscribe_current_observer(signal_id);
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
    RUNTIME.with(|runtime| runtime.tracker.record_write());
    Self::with_untracked_signal_mut_value(signal_id, action);
  }

  pub(crate) fn with_untracked_signal_mut_value<TValue>(signal_id: SignalId, action: impl FnOnce(&mut TValue))
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

  #[cfg(test)]
  pub fn subscriber_count(signal_id: SignalId) -> usize {
    RUNTIME.with(|runtime| {
      let mut count = 0;
      runtime
        .signals
        .with_subscribers(signal_id, |subscribers| count = subscribers.len());
      count
    })
  }

  pub fn is_observer_dirty(observer_id: ObserverId) -> bool {
    RUNTIME.with(|runtime| runtime.observers.is_dirty(observer_id))
  }

  pub fn create_memo(value_id: SignalId, compute: Rc<dyn Fn()>) -> MemoId {
    RUNTIME.with(|runtime| {
      let observer_id = runtime.observers.create_observer();
      runtime.observers.mark_as_dirty(&[observer_id]);
      let memo_id = runtime.memos.alloc(observer_id, value_id, compute);
      let node = ReactiveNodeId::Memo(memo_id);
      runtime.scopes.alloc_for(node, runtime.scopes.active());
      runtime.register_node(node);
      memo_id
    })
  }

  pub fn contains_memo(memo_id: MemoId) -> bool {
    RUNTIME.with(|runtime| runtime.memos.contains(memo_id))
  }

  pub fn memo_value(memo_id: MemoId) -> SignalId {
    RUNTIME.with(|runtime| runtime.memos.value(memo_id))
  }

  pub fn create_effect(run: Rc<RefCell<dyn FnMut()>>) {
    let effect_id = RUNTIME.with(|runtime| {
      let observer_id = runtime.observers.create_observer();
      runtime.observers.mark_as_dirty(&[observer_id]);
      let effect_id = runtime.effects.alloc(observer_id, run);
      let node = ReactiveNodeId::Effect(effect_id);
      runtime.scopes.alloc_for(node, runtime.scopes.active());
      runtime.register_node(node);
      effect_id
    });
    Self::refresh_effect(effect_id);
  }

  pub fn live_counts() -> LiveCounts {
    RUNTIME.with(|runtime| LiveCounts {
      signals: runtime.signals.count(),
      memos: runtime.memos.count(),
      effects: runtime.effects.count(),
      scopes: runtime.scopes.count(),
    })
  }

  fn mark_observers_as_dirty(&self, signal_id: SignalId) {
    self.signals.with_subscribers(signal_id, |subscribers| {
      self.observers.mark_as_dirty(subscribers);
    });
  }
}
