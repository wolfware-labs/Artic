use crate::arena::{Arena, SlotId};
use crate::reactive::observer::Observer;
use std::any::Any;
use std::cell::{Ref, RefCell};
use std::mem;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

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
  value: RefCell<Option<Box<dyn Any>>>,
  subscribers: RefCell<Vec<ObserverId>>,
}

impl SignalSlot {
  fn new(value: Box<dyn Any>) -> Self {
    Self {
      value: RefCell::new(Some(value)),
      subscribers: RefCell::new(Vec::new()),
    }
  }

  fn take_value(&self) -> Option<Box<dyn Any>> {
    self.value.replace(None)
  }

  fn get_value(&self) -> Ref<Option<Box<dyn Any>>> {
    self.value.borrow()
  }

  fn set_value(&self, value: Box<dyn Any>) {
    self.value.replace(Some(value));
  }

  fn add_subscriber(&self, observer: ObserverId) {
    if !self.subscribers.borrow().contains(&observer) {
      self.subscribers.borrow_mut().push(observer);
    }
  }

  fn get_subscribers(&self) -> Ref<Vec<ObserverId>> {
    self.subscribers.borrow()
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

  pub fn alloc_value<TValue>(value: TValue) -> SignalId
  where
    TValue: Any,
  {
    let slot_id = RUNTIME.with(|r| r.signals.borrow_mut().alloc(SignalSlot::new(Box::new(value))));
    SignalId(slot_id)
  }

  pub fn get_value<TValue>(id: SignalId) -> TValue
  where
    TValue: Any + Clone,
  {
    Self::with_value::<TValue, TValue>(id, |v| v.clone())
  }

  pub fn with_value<TValue, TResult>(id: SignalId, action: impl FnOnce(&TValue) -> TResult) -> TResult
  where
    TValue: Any,
  {
    RUNTIME.with(|r| {
      let arena = r.signals.borrow();
      let slot = arena.get(id.0).expect("signal slot not found for the specified id");
      r.attach_current_observer(slot);
      let value = slot.get_value();
      let Some(value) = &*value else {
        panic!("signal read while it is being updated");
      };
      action(
        value
          .downcast_ref::<TValue>()
          .expect("signal type does not match with the specified type"),
      )
    })
  }

  pub fn set_value<TValue>(id: SignalId, value: TValue)
  where
    TValue: Any,
  {
    Self::with_mut_value(id, |v| *v = value)
  }

  pub fn with_mut_value<TValue>(id: SignalId, action: impl FnOnce(&mut TValue))
  where
    TValue: Any,
  {
    RUNTIME.with(|r| {
      let arena = r.signals.borrow();
      let slot = arena.get(id.0).expect("signal slot not found for the specified id");
      let Some(mut value) = slot.take_value() else {
        panic!("signal updated while it is already being updated");
      };
      let action_result = catch_unwind(AssertUnwindSafe(|| {
        action(
          value
            .downcast_mut::<TValue>()
            .expect("signal type does not match with the specified type"),
        )
      }));
      slot.set_value(value);
      r.mark_observers_as_dirty(&slot.get_subscribers());
      if let Err(payload) = action_result {
        resume_unwind(payload);
      }
    })
  }

  fn attach_current_observer(&self, slot: &SignalSlot) {
    if let Some(observer_id) = self.stack.borrow().last()
      && !slot.subscribers.borrow().contains(observer_id)
    {
      slot.subscribers.borrow_mut().push(*observer_id)
    }
  }

  fn mark_observers_as_dirty(&self, observer_ids: &[ObserverId]) {
    let mut observers = self.observers.borrow_mut();
    for observer_id in observer_ids {
      let Some(observer) = observers.get_mut(observer_id.0) else {
        continue;
      };
      observer.mark_as_dirty();
    }
  }
}
