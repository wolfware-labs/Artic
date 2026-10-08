use crate::arena::{Arena, SlotId};
use crate::reactive::observable::ObservableId;
use crate::reactive::run_tracked;
use crate::reactive::runtime::{ObserverId, Runtime, SignalId};
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;

thread_local! {
  static MEMOS: MemoStore = const {
    MemoStore::new()
  };
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct MemoId(SlotId);

struct MemoSlot {
  observer: ObserverId,
  compute: Box<dyn Fn()>,
  value: SignalId,
  observables: RefCell<Vec<ObservableId>>,
  is_computing: Cell<bool>,
}

pub struct MemoStore {
  slots: RefCell<Arena<Rc<MemoSlot>>>,
}

impl MemoStore {
  const fn new() -> Self {
    Self {
      slots: RefCell::new(Arena::new()),
    }
  }

  pub fn create(value: SignalId, compute: Box<dyn Fn()>) -> MemoId {
    let observer = Runtime::create_observer();
    Runtime::mark_observer_as_dirty(observer);
    let slot = MemoSlot {
      observer,
      compute,
      value,
      observables: RefCell::new(Vec::new()),
      is_computing: Cell::new(false),
    };
    MEMOS.with(|memos| MemoId(memos.slots.borrow_mut().alloc(Rc::new(slot))))
  }

  pub fn value(memo_id: MemoId) -> SignalId {
    Self::get_slot(memo_id).value
  }

  pub fn refresh(memo_id: MemoId) {
    let slot = Self::get_slot(memo_id);
    if slot.is_computing.get() {
      panic!("memo read while it is being computed; a memo can't depend on itself");
    }
    let observables = slot.observables.borrow().clone();
    for observable in &observables {
      if Runtime::is_observer_dirty(slot.observer) {
        break;
      }
      observable.refresh();
    }
    if !Runtime::is_observer_dirty(slot.observer) {
      return;
    }
    for observable in observables {
      observable.unsubscribe(slot.observer);
    }
    slot.observables.borrow_mut().clear();
    slot.is_computing.set(true);
    let compute_result = catch_unwind(AssertUnwindSafe(|| run_tracked(slot.observer, &slot.compute)));
    slot.is_computing.set(false);
    match compute_result {
      Ok(observables) => *slot.observables.borrow_mut() = observables,
      Err(payload) => {
        Runtime::mark_observer_as_dirty(slot.observer);
        resume_unwind(payload);
      }
    }
  }

  fn get_slot(memo_id: MemoId) -> Rc<MemoSlot> {
    MEMOS.with(|memos| {
      let slots = memos.slots.borrow();
      Rc::clone(slots.get(memo_id.0).expect("memo slot not found for the specified id"))
    })
  }
}
