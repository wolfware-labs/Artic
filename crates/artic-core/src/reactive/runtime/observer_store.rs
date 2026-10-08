use crate::arena::{Arena, SlotId};
use std::cell::RefCell;

struct Observer {
  is_dirty: bool,
}

impl Observer {
  fn new() -> Self {
    Self { is_dirty: false }
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ObserverId(SlotId);

pub struct ObserverStore {
  observers: RefCell<Arena<Observer>>,
}

impl ObserverStore {
  pub const fn new() -> Self {
    Self {
      observers: RefCell::new(Arena::new()),
    }
  }

  pub fn create_observer(&self) -> ObserverId {
    ObserverId(self.observers.borrow_mut().alloc(Observer::new()))
  }

  pub fn free(&self, observer_id: ObserverId) {
    self.observers.borrow_mut().free(observer_id.0);
  }

  pub fn mark_as_dirty(&self, observer_ids: &[ObserverId]) {
    let mut observers = self.observers.borrow_mut();
    for observer_id in observer_ids {
      let Some(observer) = observers.get_mut(observer_id.0) else {
        continue;
      };
      observer.is_dirty = true;
    }
  }

  pub fn mark_as_clean(&self, observer_id: ObserverId) {
    let mut observers = self.observers.borrow_mut();
    observers.get_mut(observer_id.0).expect("observer does not exist").is_dirty = false;
  }

  pub fn is_dirty(&self, observer_id: ObserverId) -> bool {
    let observers = self.observers.borrow();
    observers.get(observer_id.0).expect("observer does not exist").is_dirty
  }
}
