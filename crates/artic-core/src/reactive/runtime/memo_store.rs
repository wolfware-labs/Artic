use crate::arena::{Arena, SlotId};
use crate::reactive::runtime::{Node, ObserverId, SignalId};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct MemoId(SlotId);

struct MemoSlot {
  observer: ObserverId,
  value: SignalId,
  compute: Rc<dyn Fn()>,
  observables: Rc<[Node]>,
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

  pub fn alloc(&self, observer: ObserverId, value: SignalId, compute: Rc<dyn Fn()>) -> MemoId {
    MemoId(self.slots.borrow_mut().alloc(MemoSlot {
      observer,
      value,
      compute,
      observables: Rc::from([]),
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

  pub fn observer(&self, memo_id: MemoId) -> ObserverId {
    self.with_slot(memo_id, |slot| slot.observer)
  }

  pub fn value(&self, memo_id: MemoId) -> SignalId {
    self.with_slot(memo_id, |slot| slot.value)
  }

  pub fn find_value(&self, memo_id: MemoId) -> Option<SignalId> {
    self.slots.borrow().get(memo_id.0).map(|slot| slot.value)
  }

  pub fn compute(&self, memo_id: MemoId) -> Rc<dyn Fn()> {
    self.with_slot(memo_id, |slot| Rc::clone(&slot.compute))
  }

  pub fn observables(&self, memo_id: MemoId) -> Rc<[Node]> {
    self.with_slot(memo_id, |slot| Rc::clone(&slot.observables))
  }

  pub fn set_observables(&self, memo_id: MemoId, observables: Vec<Node>) {
    self.with_slot(memo_id, |slot| slot.observables = Rc::from(observables));
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
