use crate::arena::{Arena, SlotId};
use crate::reactive::runtime::observer_store::ObserverId;
use std::any::Any;
use std::cell::{Ref, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};

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

  fn get_value(&self) -> Ref<'_, Option<Box<dyn Any>>> {
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

  fn get_subscribers(&self) -> Ref<'_, Vec<ObserverId>> {
    self.subscribers.borrow()
  }
}

pub struct SignalStore {
  slots: RefCell<Arena<SignalSlot>>,
}

impl SignalStore {
  pub const fn new() -> Self {
    Self {
      slots: RefCell::new(Arena::new()),
    }
  }

  pub fn alloc_value<TValue>(&self, value: TValue) -> SignalId
  where
    TValue: Any,
  {
    SignalId(self.slots.borrow_mut().alloc(SignalSlot::new(Box::new(value))))
  }

  pub fn with_signal_value<TValue, TResult>(&self, signal_id: SignalId, action: impl FnOnce(&TValue) -> TResult) -> TResult
  where
    TValue: Any,
  {
    let arena = self.slots.borrow();
    let slot = arena.get(signal_id.0).expect("signal slot not found for the specified id");
    let value = slot.get_value();
    let Some(value) = &*value else {
      panic!("signal read while it is being updated");
    };
    action(
      value
        .downcast_ref::<TValue>()
        .expect("signal type does not match with the specified type"),
    )
  }

  pub fn with_signal_mut_value<TValue>(&self, signal_id: SignalId, action: impl FnOnce(&mut TValue)) -> std::thread::Result<()>
  where
    TValue: Any,
  {
    let arena = self.slots.borrow();
    let slot = arena.get(signal_id.0).expect("signal slot not found for the specified id");
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
    action_result
  }

  pub fn add_subscriber(&self, signal_id: SignalId, observer_id: ObserverId) {
    if let Some(signal_slot) = self.slots.borrow().get(signal_id.0) {
      signal_slot.add_subscriber(observer_id);
    }
  }

  pub fn with_subscribers(&self, signal_id: SignalId, action: impl FnOnce(&[ObserverId])) {
    if let Some(signal_slot) = self.slots.borrow().get(signal_id.0) {
      action(&signal_slot.get_subscribers())
    }
  }
}

#[cfg(test)]
mod tests {
  // These tests inspect the private subscriber list of a slot, which is only visible from this module.

  // Subscribing the same observer to a signal twice keeps a single entry, so repeated reads in one
  // observer subscribe it only once.
  #[test]
  fn add_subscriber_ignores_duplicates() {
    todo!()
  }
}
