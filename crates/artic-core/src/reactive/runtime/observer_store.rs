use crate::arena::{Arena, SlotId};
use crate::reactive::runtime::signal_store::SignalId;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ObserverState {
  Clean,
  Check,
  Dirty,
}

struct Observer {
  state: ObserverState,
  sources: Vec<SignalId>,
  memo: Option<MemoParts>,
}

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

  pub fn take_sources(&self, observer_id: ObserverId) -> Vec<SignalId> {
    self.with_observer(observer_id, |observer| std::mem::take(&mut observer.sources))
  }

  pub fn set_memo(&self, observer_id: ObserverId, value: SignalId, refresh: Rc<dyn Fn()>) {
    self.with_observer(observer_id, |observer| observer.memo = Some(MemoParts { value, refresh }));
  }

  pub fn memo_value(&self, observer_id: ObserverId) -> Option<SignalId> {
    self.with_observer(observer_id, |observer| observer.memo.as_ref().map(|memo| memo.value))
  }

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
