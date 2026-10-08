mod memo_store;
mod observer_store;
mod signal_store;

pub use memo_store::MemoId;
use memo_store::MemoStore;
pub use observer_store::ObserverId;
use observer_store::ObserverStore;
pub use signal_store::SignalId;
use signal_store::SignalStore;
use std::any::Any;
use std::cell::RefCell;
use std::panic::resume_unwind;
use std::rc::Rc;

thread_local! {
  static RUNTIME: Runtime  = const {
    Runtime::new()
  };
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ObservableId {
  Signal(SignalId),
  Memo(MemoId),
}

pub struct Runtime {
  signals: SignalStore,
  observers: ObserverStore,
  memos: MemoStore,
  observables: RefCell<Vec<Vec<ObservableId>>>,
}

impl Runtime {
  const fn new() -> Self {
    Self {
      signals: SignalStore::new(),
      observers: ObserverStore::new(),
      memos: MemoStore::new(),
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

  pub fn unsubscribe(signal_id: SignalId, observer_id: ObserverId) {
    RUNTIME.with(|runtime| runtime.signals.remove_subscriber(signal_id, observer_id))
  }

  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by Effect (#10); only tests call it until then")
  )]
  pub fn create_observer() -> ObserverId {
    RUNTIME.with(|runtime| runtime.observers.create_observer())
  }

  pub fn push_observer(observer_id: ObserverId) {
    RUNTIME.with(|runtime| {
      runtime.observers.push_observer(observer_id);
      runtime.observables.borrow_mut().push(Vec::new());
    })
  }

  pub fn pop_observer() -> Vec<ObservableId> {
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

  pub fn mark_observer_as_clean(observer_id: ObserverId) {
    RUNTIME.with(|runtime| runtime.observers.mark_as_clean(observer_id))
  }

  pub fn mark_observer_as_dirty(observer_id: ObserverId) {
    RUNTIME.with(|runtime| runtime.observers.mark_as_dirty(&[observer_id]))
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

  pub fn memo_observer(memo_id: MemoId) -> ObserverId {
    RUNTIME.with(|runtime| runtime.memos.observer(memo_id))
  }

  pub fn memo_value(memo_id: MemoId) -> SignalId {
    RUNTIME.with(|runtime| runtime.memos.value(memo_id))
  }

  pub fn memo_compute(memo_id: MemoId) -> Rc<dyn Fn()> {
    RUNTIME.with(|runtime| runtime.memos.compute(memo_id))
  }

  pub fn memo_observables(memo_id: MemoId) -> Vec<ObservableId> {
    RUNTIME.with(|runtime| runtime.memos.observables(memo_id))
  }

  pub fn set_memo_observables(memo_id: MemoId, observables: Vec<ObservableId>) {
    RUNTIME.with(|runtime| runtime.memos.set_observables(memo_id, observables))
  }

  pub fn is_memo_computing(memo_id: MemoId) -> bool {
    RUNTIME.with(|runtime| runtime.memos.is_computing(memo_id))
  }

  pub fn set_memo_computing(memo_id: MemoId, is_computing: bool) {
    RUNTIME.with(|runtime| runtime.memos.set_computing(memo_id, is_computing))
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
