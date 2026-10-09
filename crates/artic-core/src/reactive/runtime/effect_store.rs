use crate::arena::{Arena, SlotId};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct EffectId(SlotId);

struct EffectSlot {
  seq: u64,
  run: Rc<RefCell<dyn FnMut()>>,
  is_dirty: bool,
  is_running: bool,
}

pub struct EffectStore {
  slots: RefCell<Arena<EffectSlot>>,
  next_seq: Cell<u64>,
}

impl EffectStore {
  pub const fn new() -> Self {
    Self {
      slots: RefCell::new(Arena::new()),
      next_seq: Cell::new(0),
    }
  }

  pub fn alloc(&self, run: Rc<RefCell<dyn FnMut()>>) -> EffectId {
    let seq = self.next_seq.replace(self.next_seq.get() + 1);
    EffectId(self.slots.borrow_mut().alloc(EffectSlot {
      seq,
      run,
      is_dirty: true,
      is_running: false,
    }))
  }

  pub fn free(&self, effect_id: EffectId) {
    self.slots.borrow_mut().free(effect_id.0);
  }

  pub fn contains(&self, effect_id: EffectId) -> bool {
    self.slots.borrow().contains(effect_id.0)
  }

  pub fn count(&self) -> usize {
    self.slots.borrow().len()
  }

  pub fn seq(&self, effect_id: EffectId) -> u64 {
    self.with_slot(effect_id, |slot| slot.seq)
  }

  pub fn run(&self, effect_id: EffectId) -> Rc<RefCell<dyn FnMut()>> {
    self.with_slot(effect_id, |slot| Rc::clone(&slot.run))
  }

  pub fn is_dirty(&self, effect_id: EffectId) -> bool {
    self.with_slot(effect_id, |slot| slot.is_dirty)
  }

  pub fn set_dirty(&self, effect_id: EffectId, is_dirty: bool) {
    self.with_slot(effect_id, |slot| slot.is_dirty = is_dirty);
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
