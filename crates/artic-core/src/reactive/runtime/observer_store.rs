use crate::arena::{Arena, SlotId};
use crate::reactive::runtime::signal_store::SignalId;
use std::cell::RefCell;
use std::rc::Rc;

/// How up to date an observer's last result is.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ObserverState {
  /// The last result is current.
  Clean,
  /// A memo this observer depends on, directly or through other memos, may have changed. The
  /// observer is out of date only if refreshing those memos changes their values.
  Check,
  /// A source changed, so the last result is out of date.
  Dirty,
}

struct Observer {
  state: ObserverState,
  /// The signals read during the last run. Kept so the next run can unsubscribe from all of them
  /// before recording a fresh set.
  sources: Vec<SignalId>,
  memo: Option<MemoParts>,
}

/// What turns an observer into a memo: the signal slot that caches its value, and the closure that
/// recomputes that value.
struct MemoParts {
  value: SignalId,
  refresh: Rc<dyn Fn()>,
}

impl Observer {
  fn new(state: ObserverState) -> Self {
    Self {
      state,
      sources: Vec::new(),
      memo: None,
    }
  }

  fn add_source(&mut self, signal_id: SignalId) {
    if !self.sources.contains(&signal_id) {
      self.sources.push(signal_id);
    }
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ObserverId(SlotId);

pub struct ObserverStore {
  observers: RefCell<Arena<Observer>>,
  stack: RefCell<Vec<ObserverId>>,
}

impl ObserverStore {
  pub const fn new() -> Self {
    Self {
      observers: RefCell::new(Arena::new()),
      stack: RefCell::new(Vec::new()),
    }
  }

  pub fn create_observer(&self, state: ObserverState) -> ObserverId {
    ObserverId(self.observers.borrow_mut().alloc(Observer::new(state)))
  }

  pub fn push_observer(&self, observer: ObserverId) {
    self.stack.borrow_mut().push(observer);
  }

  pub fn pop_observer(&self) {
    self.stack.borrow_mut().pop();
  }

  pub fn current_observer(&self) -> Option<ObserverId> {
    self.stack.borrow().last().copied()
  }

  pub fn is_running(&self, observer_id: ObserverId) -> bool {
    self.stack.borrow().contains(&observer_id)
  }

  pub fn state(&self, observer_id: ObserverId) -> ObserverState {
    self.with_observer(observer_id, |observer| observer.state)
  }

  pub fn set_state(&self, observer_id: ObserverId, state: ObserverState) {
    self.with_observer(observer_id, |observer| observer.state = state);
  }

  pub fn add_source(&self, observer_id: ObserverId, signal_id: SignalId) {
    self.with_observer(observer_id, |observer| observer.add_source(signal_id));
  }

  pub fn sources(&self, observer_id: ObserverId) -> Vec<SignalId> {
    self.with_observer(observer_id, |observer| observer.sources.clone())
  }

  /// Removes and returns the sources recorded by the last run.
  pub fn take_sources(&self, observer_id: ObserverId) -> Vec<SignalId> {
    self.with_observer(observer_id, |observer| std::mem::take(&mut observer.sources))
  }

  pub fn set_memo(&self, observer_id: ObserverId, value: SignalId, refresh: Rc<dyn Fn()>) {
    self.with_observer(observer_id, |observer| observer.memo = Some(MemoParts { value, refresh }));
  }

  /// The signal slot holding the value of `observer_id`, if the observer is a memo.
  pub fn memo_value(&self, observer_id: ObserverId) -> Option<SignalId> {
    self.with_observer(observer_id, |observer| observer.memo.as_ref().map(|memo| memo.value))
  }

  /// The closure that recomputes `observer_id`, if the observer is a memo. The `Rc` is cloned so the
  /// caller can run it after the store's borrow ends: the closure reads signals and other memos,
  /// which borrow the store again.
  pub fn memo_refresh(&self, observer_id: ObserverId) -> Option<Rc<dyn Fn()>> {
    self.with_observer(observer_id, |observer| {
      observer.memo.as_ref().map(|memo| Rc::clone(&memo.refresh))
    })
  }

  fn with_observer<TResult>(&self, observer_id: ObserverId, action: impl FnOnce(&mut Observer) -> TResult) -> TResult {
    let mut observers = self.observers.borrow_mut();
    action(observers.get_mut(observer_id.0).expect("observer does not exist"))
  }
}
