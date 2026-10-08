use crate::arena::{Arena, SlotId};
use crate::reactive::runtime::{ObservableId, ObserverId};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct EffectId(SlotId);

struct EffectSlot {
  observer: ObserverId,
  run: Rc<RefCell<dyn FnMut()>>,
  observables: Rc<[ObservableId]>,
  is_running: bool,
}

pub struct EffectStore {
  slots: RefCell<Arena<EffectSlot>>,
  creation_order: RefCell<Vec<EffectId>>,
}

impl EffectStore {
  pub const fn new() -> Self {
    Self {
      slots: RefCell::new(Arena::new()),
      creation_order: RefCell::new(Vec::new()),
    }
  }

  pub fn alloc(&self, observer: ObserverId, run: Rc<RefCell<dyn FnMut()>>) -> EffectId {
    let effect_id = EffectId(self.slots.borrow_mut().alloc(EffectSlot {
      observer,
      run,
      observables: Rc::from([]),
      is_running: false,
    }));
    self.creation_order.borrow_mut().push(effect_id);
    effect_id
  }

  pub fn free(&self, effect_id: EffectId) {
    self.slots.borrow_mut().free(effect_id.0);
  }

  pub fn remove_from_creation_order(&self, effect_id: EffectId) {
    self.creation_order.borrow_mut().retain(|created| *created != effect_id);
  }

  pub fn compact_creation_order(&self) {
    let slots = self.slots.borrow();
    self.creation_order.borrow_mut().retain(|created| slots.contains(created.0));
  }

  pub fn contains(&self, effect_id: EffectId) -> bool {
    self.slots.borrow().contains(effect_id.0)
  }

  pub fn count(&self) -> usize {
    self.slots.borrow().len()
  }

  pub fn nth_created(&self, index: usize) -> Option<EffectId> {
    self.creation_order.borrow().get(index).copied()
  }

  pub fn observer(&self, effect_id: EffectId) -> ObserverId {
    self.with_slot(effect_id, |slot| slot.observer)
  }

  pub fn run(&self, effect_id: EffectId) -> Rc<RefCell<dyn FnMut()>> {
    self.with_slot(effect_id, |slot| Rc::clone(&slot.run))
  }

  pub fn observables(&self, effect_id: EffectId) -> Rc<[ObservableId]> {
    self.with_slot(effect_id, |slot| Rc::clone(&slot.observables))
  }

  pub fn set_observables(&self, effect_id: EffectId, observables: Vec<ObservableId>) {
    self.with_slot(effect_id, |slot| slot.observables = Rc::from(observables));
  }

  pub fn is_running(&self, effect_id: EffectId) -> bool {
    self.with_slot(effect_id, |slot| slot.is_running)
  }

  pub fn set_running(&self, effect_id: EffectId, is_running: bool) {
    self.with_slot(effect_id, |slot| slot.is_running = is_running);
  }

  fn with_slot<TResult>(&self, effect_id: EffectId, action: impl FnOnce(&mut EffectSlot) -> TResult) -> TResult {
    let mut slots = self.slots.borrow_mut();
    action(
      slots
        .get_mut(effect_id.0)
        .expect("effect slot not found for the specified id"),
    )
  }
}
