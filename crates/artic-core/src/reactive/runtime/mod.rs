mod observer_store;
mod signal_store;

pub use observer_store::ObserverId;
pub use observer_store::ObserverState;
use observer_store::ObserverStore;
pub use signal_store::SignalId;
use signal_store::SignalStore;
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;

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
    RUNTIME.with(|runtime| runtime.signals.alloc_value(value, None))
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
    expect(dead_code, reason = "used by Effect (#10); only tests call it until then")
  )]
  pub fn create_observer() -> ObserverId {
    RUNTIME.with(|runtime| runtime.observers.create_observer(ObserverState::Clean))
  }

  pub fn reset_observer(observer_id: ObserverId) {
    RUNTIME.with(|runtime| {
      for signal_id in runtime.observers.take_sources(observer_id) {
        runtime.signals.remove_subscriber(signal_id, observer_id);
      }
      runtime.observers.set_state(observer_id, ObserverState::Clean);
    })
  }

  pub fn push_observer(observer_id: ObserverId) {
    RUNTIME.with(|runtime| runtime.observers.push_observer(observer_id))
  }

  pub fn pop_observer() {
    RUNTIME.with(|runtime| runtime.observers.pop_observer())
  }

  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by Effect (#10); only tests call it until then")
  )]
  pub fn is_observer_dirty(observer_id: ObserverId) -> bool {
    RUNTIME.with(|runtime| runtime.observers.state(observer_id) == ObserverState::Dirty)
  }

  pub fn create_memo<TValue>(
    initial_value: TValue,
    make_refresh: impl FnOnce(ObserverId, SignalId) -> Rc<dyn Fn()>,
  ) -> (ObserverId, SignalId)
  where
    TValue: Any,
  {
    let (observer_id, value_id) = RUNTIME.with(|runtime| {
      let observer_id = runtime.observers.create_observer(ObserverState::Dirty);
      let value_id = runtime.signals.alloc_value(initial_value, Some(observer_id));
      (observer_id, value_id)
    });
    let refresh = make_refresh(observer_id, value_id);
    RUNTIME.with(|runtime| runtime.observers.set_memo(observer_id, value_id, refresh));
    (observer_id, value_id)
  }

  pub fn refresh_memo(observer_id: ObserverId) {
    if RUNTIME.with(|runtime| runtime.observers.is_running(observer_id)) {
      panic!("memo read while it is being computed; a memo can't depend on itself");
    }
    if !Self::needs_rerun(observer_id) {
      return;
    }
    let refresh = RUNTIME
      .with(|runtime| runtime.observers.memo_refresh(observer_id))
      .expect("observer is not a memo");
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| refresh())) {
      RUNTIME.with(|runtime| runtime.observers.set_state(observer_id, ObserverState::Dirty));
      resume_unwind(payload);
    }
  }

  pub fn needs_rerun(observer_id: ObserverId) -> bool {
    let state = RUNTIME.with(|runtime| runtime.observers.state(observer_id));
    match state {
      ObserverState::Clean => false,
      ObserverState::Dirty => true,
      ObserverState::Check => {
        let sources = RUNTIME.with(|runtime| runtime.observers.sources(observer_id));
        for source in sources {
          if let Some(memo) = RUNTIME.with(|runtime| runtime.signals.owner(source)) {
            Self::refresh_memo(memo);
          }
          if RUNTIME.with(|runtime| runtime.observers.state(observer_id)) == ObserverState::Dirty {
            return true;
          }
        }
        RUNTIME.with(|runtime| runtime.observers.set_state(observer_id, ObserverState::Clean));
        false
      }
    }
  }

  fn attach_current_observer(&self, signal_id: SignalId) {
    if let Some(observer_id) = self.observers.current_observer() {
      self.signals.add_subscriber(signal_id, observer_id);
      self.observers.add_source(observer_id, signal_id);
    }
  }

  fn mark_observers_as_dirty(&self, signal_id: SignalId) {
    for observer_id in self.signals.subscribers(signal_id) {
      self.observers.set_state(observer_id, ObserverState::Dirty);
      if let Some(value_id) = self.observers.memo_value(observer_id) {
        self.mark_downstream_as_check(value_id);
      }
    }
  }

  fn mark_downstream_as_check(&self, signal_id: SignalId) {
    let mut pending = vec![signal_id];
    while let Some(signal_id) = pending.pop() {
      for observer_id in self.signals.subscribers(signal_id) {
        if self.observers.state(observer_id) != ObserverState::Clean {
          continue;
        }
        self.observers.set_state(observer_id, ObserverState::Check);
        if let Some(value_id) = self.observers.memo_value(observer_id) {
          pending.push(value_id);
        }
      }
    }
  }
}
