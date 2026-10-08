use crate::arena::{Arena, SlotId};
use std::cell::RefCell;

struct Observer {
  is_dirty: bool,
}

impl Observer {
  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by Memo and Effect (#9, #10); only tests call it until then")
  )]
  fn new() -> Self {
    Self { is_dirty: false }
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

  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by Memo and Effect (#9, #10); only tests call it until then")
  )]
  pub fn create_observer(&self) -> ObserverId {
    ObserverId(self.observers.borrow_mut().alloc(Observer::new()))
  }

  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by Memo and Effect (#9, #10); only tests call it until then")
  )]
  pub fn push_observer(&self, observer: ObserverId) {
    self.stack.borrow_mut().push(observer);
  }

  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by Memo and Effect (#9, #10); only tests call it until then")
  )]
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
      observer.is_dirty = true;
    }
  }

  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by Memo and Effect (#9, #10); only tests call it until then")
  )]
  pub fn mark_as_clean(&self, observer_id: ObserverId) {
    let mut observers = self.observers.borrow_mut();
    observers.get_mut(observer_id.0).expect("observer does not exist").is_dirty = false;
  }

  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by Memo and Effect (#9, #10); only tests call it until then")
  )]
  pub fn is_dirty(&self, observer_id: ObserverId) -> bool {
    let observers = self.observers.borrow();
    observers.get(observer_id.0).expect("observer does not exist").is_dirty
  }
}
