use crate::arena::{Arena, SlotId};
use crate::reactive::observer::Observer;
use std::any::Any;
use std::cell::RefCell;

thread_local! {
  static RUNTIME: Runtime  = const {
    Runtime::new()
  };
}

pub struct Runtime {
  signals: RefCell<Arena<SignalSlot>>,
  observers: RefCell<Arena<Observer>>,
  stack: RefCell<Vec<ObserverId>>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct SignalId(SlotId);

struct SignalSlot {
  value: Box<dyn Any>,
  subscribers: Vec<ObserverId>,
}

impl SignalSlot {
  fn new(value: Box<dyn Any>) -> Self {
    Self {
      value,
      subscribers: Vec::new(),
    }
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ObserverId(SlotId);

impl Runtime {
  const fn new() -> Self {
    Self {
      signals: RefCell::new(Arena::new()),
      observers: RefCell::new(Arena::new()),
      stack: RefCell::new(Vec::new()),
    }
  }

  pub fn alloc_observer(observer: Observer) -> ObserverId {
    let slot_id = RUNTIME.with(|r| r.observers.borrow_mut().alloc(observer));
    ObserverId(slot_id)
  }

  pub fn push_observer(observer: ObserverId) {
    RUNTIME.with(|r| r.stack.borrow_mut().push(observer))
  }

  pub fn pop_observer() {
    RUNTIME.with(|r| r.stack.borrow_mut().pop());
  }

  pub fn alloc_value<T>(value: T) -> SignalId
  where
    T: Any,
  {
    let slot_id = RUNTIME.with(|r| r.signals.borrow_mut().alloc(SignalSlot::new(Box::new(value))));
    SignalId(slot_id)
  }

  pub fn get_value<T>(id: SignalId) -> T
  where
    T: Any + Clone,
  {
    RUNTIME.with(|r| {
      let mut arena = r.signals.borrow_mut();
      let slot = arena.get_mut(id.0).expect("signal slot not found for the specified id");
      Self::attach_current_observer(slot);
      slot
        .value
        .downcast_ref::<T>()
        .expect("signal type does not match with the specified type")
        .clone()
    })
  }

  pub fn with_value<T>(id: SignalId, action: impl FnOnce(&T))
  where
    T: Any,
  {
    RUNTIME.with(|r| {
      let mut arena = r.signals.borrow_mut();
      let slot = arena.get_mut(id.0).expect("signal slot not found for the specified id");
      Self::attach_current_observer(slot);
      action(
        slot
          .value
          .downcast_ref::<T>()
          .expect("signal type does not match with the specified type"),
      );
    })
  }

  pub fn set_value<T>(id: SignalId, value: T)
  where
    T: Any,
  {
    RUNTIME.with(|r| {
      let mut arena = r.signals.borrow_mut();
      let slot = arena.get_mut(id.0);
      let Some(slot) = slot else {
        return;
      };
      slot.value = Box::new(value);
      Self::mark_observers_as_dirty(&slot.subscribers);
    })
  }

  pub fn with_mut_value<T>(id: SignalId, action: impl FnOnce(&mut T))
  where
    T: Any,
  {
    RUNTIME.with(|r| {
      let mut arena = r.signals.borrow_mut();
      let slot = arena.get_mut(id.0).expect("signal slot not found for the specified id");
      action(
        slot
          .value
          .downcast_mut::<T>()
          .expect("signal type does not match with the specified type"),
      );
      Self::mark_observers_as_dirty(&slot.subscribers);
    })
  }

  fn attach_current_observer(slot: &mut SignalSlot) {
    RUNTIME.with(|r| {
      if let Some(observer_id) = r.stack.borrow().last()
        && !slot.subscribers.contains(observer_id)
      {
        slot.subscribers.push(observer_id.clone())
      }
    })
  }

  fn mark_observers_as_dirty(observer_ids: &[ObserverId]) {
    RUNTIME.with(|r| {
      let mut observers = r.observers.borrow_mut();
      for observer_id in observer_ids {
        let Some(observer) = observers.get_mut(observer_id.0) else {
          continue;
        };
        observer.mark_as_dirty();
      }
    });
  }
}
