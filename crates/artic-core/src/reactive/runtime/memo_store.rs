use crate::arena::{Arena, SlotId};
use crate::reactive::runtime::value_cell::ValueCell;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct MemoId(SlotId);

struct MemoSlot {
  value: Rc<ValueCell>,
  compute: Rc<dyn Fn(MemoId)>,
  is_dirty: bool,
  is_computing: bool,
}

pub struct MemoStore {
  slots: RefCell<Arena<MemoSlot>>,
}

impl MemoStore {
  pub const fn new() -> Self {
    Self {
      slots: RefCell::new(Arena::new()),
    }
  }

  pub fn alloc(&self, value: Box<dyn Any>, compute: Rc<dyn Fn(MemoId)>) -> MemoId {
    MemoId(self.slots.borrow_mut().alloc(MemoSlot {
      value: Rc::new(ValueCell::new(value, "memo")),
      compute,
      is_dirty: true,
      is_computing: false,
    }))
  }

  pub fn free(&self, memo_id: MemoId) {
    self.slots.borrow_mut().free(memo_id.0);
  }

  pub fn contains(&self, memo_id: MemoId) -> bool {
    self.slots.borrow().contains(memo_id.0)
  }

  pub fn count(&self) -> usize {
    self.slots.borrow().len()
  }

  pub fn value(&self, memo_id: MemoId) -> Rc<ValueCell> {
    self.with_slot(memo_id, |slot| Rc::clone(&slot.value))
  }

  pub fn compute(&self, memo_id: MemoId) -> Rc<dyn Fn(MemoId)> {
    self.with_slot(memo_id, |slot| Rc::clone(&slot.compute))
  }

  pub fn is_dirty(&self, memo_id: MemoId) -> bool {
    self.with_slot(memo_id, |slot| slot.is_dirty)
  }

  pub fn set_dirty(&self, memo_id: MemoId, is_dirty: bool) {
    self.with_slot(memo_id, |slot| slot.is_dirty = is_dirty);
  }

  pub fn is_computing(&self, memo_id: MemoId) -> bool {
    self.with_slot(memo_id, |slot| slot.is_computing)
  }

  pub fn set_computing(&self, memo_id: MemoId, is_computing: bool) {
    self.with_slot(memo_id, |slot| slot.is_computing = is_computing);
  }

  fn with_slot<TResult>(&self, memo_id: MemoId, action: impl FnOnce(&mut MemoSlot) -> TResult) -> TResult {
    let mut slots = self.slots.borrow_mut();
    action(slots.get_mut(memo_id.0).expect("memo slot not found for the specified id"))
  }
}
