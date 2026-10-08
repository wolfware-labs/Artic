use crate::arena::{Arena, SlotId};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ScopeId(SlotId);

struct ScopeSlot {
  #[expect(dead_code, reason = "dispose detaches a scope from its parent (#11)")]
  parent: Option<ScopeId>,
  children: Vec<ScopeId>,
}

pub struct ScopeStore {
  slots: RefCell<Arena<ScopeSlot>>,
  active: RefCell<Vec<ScopeId>>,
}

impl ScopeStore {
  pub const fn new() -> Self {
    Self {
      slots: RefCell::new(Arena::new()),
      active: RefCell::new(Vec::new()),
    }
  }

  pub fn alloc(&self, parent: Option<ScopeId>) -> ScopeId {
    let mut slots = self.slots.borrow_mut();
    let scope_id = ScopeId(slots.alloc(ScopeSlot {
      parent,
      children: Vec::new(),
    }));
    if let Some(parent) = parent {
      slots
        .get_mut(parent.0)
        .expect("scope slot not found for the specified id")
        .children
        .push(scope_id);
    }
    scope_id
  }

  pub fn count(&self) -> usize {
    self.slots.borrow().len()
  }

  pub fn push_active(&self, scope_id: ScopeId) {
    self.active.borrow_mut().push(scope_id);
  }

  pub fn pop_active(&self) {
    self.active.borrow_mut().pop();
  }

  #[cfg(test)]
  pub fn active(&self) -> Option<ScopeId> {
    self.active.borrow().last().copied()
  }
}
