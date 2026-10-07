use crate::arena::{Arena, SlotId};
use crate::reactive::runtime::signal_store::SignalId;
use std::cell::RefCell;

/// How up to date an observer's last result is.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ObserverState {
  /// The last result is current.
  Clean,
  /// A source changed, so the last result is out of date.
  Dirty,
}

struct Observer {
  state: ObserverState,
  /// The signals read during the last run. Kept so the next run can unsubscribe from all of them
  /// before recording a fresh set.
  sources: Vec<SignalId>,
}

impl Observer {
  fn new(state: ObserverState) -> Self {
    Self {
      state,
      sources: Vec::new(),
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

  pub fn mark_as_dirty(&self, observer_ids: &[ObserverId]) {
    let mut observers = self.observers.borrow_mut();
    for observer_id in observer_ids {
      let Some(observer) = observers.get_mut(observer_id.0) else {
        continue;
      };
      observer.state = ObserverState::Dirty;
    }
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

  /// Removes and returns the sources recorded by the last run.
  pub fn take_sources(&self, observer_id: ObserverId) -> Vec<SignalId> {
    self.with_observer(observer_id, |observer| std::mem::take(&mut observer.sources))
  }

  fn with_observer<TResult>(&self, observer_id: ObserverId, action: impl FnOnce(&mut Observer) -> TResult) -> TResult {
    let mut observers = self.observers.borrow_mut();
    action(observers.get_mut(observer_id.0).expect("observer does not exist"))
  }
}
