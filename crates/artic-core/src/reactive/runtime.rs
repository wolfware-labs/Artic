mod effect_store;
mod memo_store;
mod observable;
mod observer_store;
mod refresh;
mod signal_store;
mod tracking;

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
use std::cell::{Cell, RefCell};
use std::panic::resume_unwind;
use std::rc::Rc;
use std::thread;
use tracking::Tracker;

const MAX_FLUSH_PASSES: usize = 100;

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
  tracker: Tracker,
  has_changes: Cell<bool>,
  is_flushing: Cell<bool>,
  batch_depth: Cell<usize>,
}

impl Runtime {
  const fn new() -> Self {
    Self {
      signals: SignalStore::new(),
      observers: ObserverStore::new(),
      memos: MemoStore::new(),
      effects: EffectStore::new(),
      tracker: Tracker::new(),
      has_changes: Cell::new(false),
      is_flushing: Cell::new(false),
      batch_depth: Cell::new(0),
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
    RUNTIME.with(|runtime| runtime.has_changes.set(true));
    Self::write_signal_value(signal_id, action);
  }

  pub fn set_memo_value<TValue>(value_id: SignalId, value: TValue)
  where
    TValue: Any,
  {
    Self::write_signal_value(value_id, |v| *v = value);
  }

  fn write_signal_value<TValue>(signal_id: SignalId, action: impl FnOnce(&mut TValue))
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

  pub fn create_effect(run: Rc<RefCell<dyn FnMut()>>) -> EffectId {
    let effect_id = RUNTIME.with(|runtime| {
      let observer_id = runtime.observers.create_observer();
      runtime.observers.mark_as_dirty(&[observer_id]);
      runtime.effects.alloc(observer_id, run)
    });
    Self::refresh_effect(effect_id);
    effect_id
  }

  pub fn batch<TResult>(action: impl FnOnce() -> TResult) -> TResult {
    let result = {
      let _batch = BatchGuard::enter();
      action()
    };
    Self::flush_effects();
    result
  }

  pub fn flush_effects() {
    if RUNTIME.with(|runtime| runtime.batch_depth.get() > 0 || runtime.is_flushing.replace(true)) {
      return;
    }
    let _flushing = FlushingGuard;
    for _ in 0..MAX_FLUSH_PASSES {
      if !RUNTIME.with(|runtime| runtime.has_changes.replace(false)) {
        return;
      }
      let mut index = 0;
      while let Some(effect_id) = RUNTIME.with(|runtime| runtime.effects.nth_created(index)) {
        Self::refresh_effect(effect_id);
        index += 1;
      }
    }
    panic!("effects did not settle after {MAX_FLUSH_PASSES} passes; two effects probably write each other's inputs");
  }

  fn mark_observers_as_dirty(&self, signal_id: SignalId) {
    self.signals.with_subscribers(signal_id, |subscribers| {
      self.observers.mark_as_dirty(subscribers);
    });
  }
}

struct FlushingGuard;

impl Drop for FlushingGuard {
  fn drop(&mut self) {
    RUNTIME.with(|runtime| {
      runtime.is_flushing.set(false);
      if thread::panicking() {
        runtime.has_changes.set(true);
      }
    });
  }
}

struct BatchGuard;

impl BatchGuard {
  fn enter() -> Self {
    RUNTIME.with(|runtime| runtime.batch_depth.set(runtime.batch_depth.get() + 1));
    BatchGuard
  }
}

impl Drop for BatchGuard {
  fn drop(&mut self) {
    RUNTIME.with(|runtime| runtime.batch_depth.set(runtime.batch_depth.get() - 1));
  }
}
