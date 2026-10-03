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
  stack: RefCell<Vec<Observer>>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct SignalId(SlotId);

struct SignalSlot {
  value: Box<dyn Any>,
  subscribers: Vec<Observer>,
}

impl SignalSlot {
  fn new(value: Box<dyn Any>) -> Self {
    Self {
      value,
      subscribers: Vec::new(),
    }
  }
}

impl Runtime {
  const fn new() -> Self {
    Self {
      signals: RefCell::new(Arena::new()),
      stack: RefCell::new(Vec::new()),
    }
  }

  pub fn push_observer(observer: Observer) {
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

  pub fn get_value<T>(id: SignalId) -> Option<T>
  where
    T: Any + Clone,
  {
    RUNTIME.with(|r| {
      let mut arena = r.signals.borrow_mut();
      let slot = arena.get_mut(id.0)?;
      if let Some(observer) = r.stack.borrow().last()
        && !slot.subscribers.contains(observer)
      {
        slot.subscribers.push(observer)
      }
      Some(slot.value.downcast_ref::<T>()?.clone())
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
      for subscriber in slot.subscribers.iter_mut() {
        subscriber.mark_as_dirty()
      }
    })
  }
}
