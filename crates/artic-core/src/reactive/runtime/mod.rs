mod observer_store;
mod signal_store;

pub use observer_store::ObserverId;
use observer_store::ObserverStore;
pub use signal_store::SignalId;
use signal_store::SignalStore;
use std::any::Any;
use std::panic::resume_unwind;

thread_local! {
  static RUNTIME: Runtime  = const {
    Runtime::new()
  };
}

pub struct Runtime {
  signals: SignalStore,
  observers: ObserverStore,
}

impl Runtime {
  const fn new() -> Self {
    Self {
      signals: SignalStore::new(),
      observers: ObserverStore::new(),
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

  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by Memo and Effect (#9, #10); only tests call it until then")
  )]
  pub fn create_observer() -> ObserverId {
    RUNTIME.with(|runtime| runtime.observers.create_observer())
  }

  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by Memo and Effect (#9, #10); only tests call it until then")
  )]
  pub fn push_observer(observer_id: ObserverId) {
    RUNTIME.with(|runtime| runtime.observers.push_observer(observer_id))
  }

  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by Memo and Effect (#9, #10); only tests call it until then")
  )]
  pub fn pop_observer() {
    RUNTIME.with(|runtime| runtime.observers.pop_observer())
  }

  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by Memo and Effect (#9, #10); only tests call it until then")
  )]
  pub fn is_observer_dirty(observer_id: ObserverId) -> bool {
    RUNTIME.with(|runtime| runtime.observers.is_dirty(observer_id))
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
