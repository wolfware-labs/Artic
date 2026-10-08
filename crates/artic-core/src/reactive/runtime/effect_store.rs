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
}

impl EffectStore {
  pub const fn new() -> Self {
    Self {
      slots: RefCell::new(Arena::new()),
    }
  }

  pub fn alloc(&self, observer: ObserverId, run: Rc<RefCell<dyn FnMut()>>) -> EffectId {
    EffectId(self.slots.borrow_mut().alloc(EffectSlot {
      observer,
      run,
      observables: Rc::from([]),
      is_running: false,
    }))
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
