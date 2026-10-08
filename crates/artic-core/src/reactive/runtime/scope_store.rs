use crate::arena::{Arena, SlotId};
use crate::reactive::runtime::{EffectId, MemoId, SignalId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, DefaultHasher};
use std::mem::take;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ScopeId(SlotId);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ReactiveNodeId {
  Signal(SignalId),
  Memo(MemoId),
  Effect(EffectId),
}

struct ScopeSlot {
  parent: Option<ScopeId>,
  children: Vec<ScopeId>,
  nodes: Vec<ReactiveNodeId>,
}

pub struct ScopeStore {
  slots: RefCell<Arena<ScopeSlot>>,
  active: RefCell<Vec<ScopeId>>,
  node_scopes: RefCell<HashMap<ReactiveNodeId, ScopeId, BuildHasherDefault<DefaultHasher>>>,
}

impl ScopeStore {
  pub const fn new() -> Self {
    Self {
      slots: RefCell::new(Arena::new()),
      active: RefCell::new(Vec::new()),
      node_scopes: RefCell::new(HashMap::with_hasher(BuildHasherDefault::new())),
    }
  }

  pub fn alloc(&self, parent: Option<ScopeId>) -> ScopeId {
    let scope_id = ScopeId(self.slots.borrow_mut().alloc(ScopeSlot {
      parent,
      children: Vec::new(),
      nodes: Vec::new(),
    }));
    if let Some(parent) = parent {
      self.with_slot(parent, |slot| slot.children.push(scope_id));
    }
    scope_id
  }

  pub fn alloc_for(&self, node: ReactiveNodeId, parent: Option<ScopeId>) -> ScopeId {
    let scope_id = self.alloc(parent);
    self.node_scopes.borrow_mut().insert(node, scope_id);
    scope_id
  }

  pub fn scope_of(&self, node: ReactiveNodeId) -> ScopeId {
    *self
      .node_scopes
      .borrow()
      .get(&node)
      .expect("scope not found for the specified node")
  }

  pub fn take_scope_of(&self, node: ReactiveNodeId) -> Option<ScopeId> {
    self.node_scopes.borrow_mut().remove(&node)
  }

  pub fn free(&self, scope_id: ScopeId) {
    self.slots.borrow_mut().free(scope_id.0);
  }

  pub fn contains(&self, scope_id: ScopeId) -> bool {
    self.slots.borrow().contains(scope_id.0)
  }

  pub fn count(&self) -> usize {
    self.slots.borrow().len()
  }

  pub fn parent(&self, scope_id: ScopeId) -> Option<ScopeId> {
    self.with_slot(scope_id, |slot| slot.parent)
  }

  pub fn remove_child(&self, scope_id: ScopeId, child_id: ScopeId) {
    if let Some(slot) = self.slots.borrow_mut().get_mut(scope_id.0) {
      slot.children.retain(|child| *child != child_id);
    }
  }

  pub fn take_children(&self, scope_id: ScopeId) -> Vec<ScopeId> {
    self.with_slot(scope_id, |slot| take(&mut slot.children))
  }

  pub fn add_node(&self, scope_id: ScopeId, node: ReactiveNodeId) {
    self.with_slot(scope_id, |slot| slot.nodes.push(node));
  }

  pub fn take_nodes(&self, scope_id: ScopeId) -> Vec<ReactiveNodeId> {
    self.with_slot(scope_id, |slot| take(&mut slot.nodes))
  }

  pub fn push_active(&self, scope_id: ScopeId) {
    self.active.borrow_mut().push(scope_id);
  }

  pub fn pop_active(&self) {
    self.active.borrow_mut().pop();
  }

  pub fn active(&self) -> Option<ScopeId> {
    self.active.borrow().last().copied()
  }

  fn with_slot<TResult>(&self, scope_id: ScopeId, action: impl FnOnce(&mut ScopeSlot) -> TResult) -> TResult {
    let mut slots = self.slots.borrow_mut();
    action(slots.get_mut(scope_id.0).expect("scope slot not found for the specified id"))
  }
}

#[cfg(test)]
mod tests {
  // These tests inspect the private node-to-scope map, which is only visible from this module.
  use super::*;
  use crate::reactive::runtime::signal_store::SignalStore;
  use pretty_assertions::assert_eq;

  // Taking a node's scope removes the map entry, so a disposed node leaves nothing behind.
  #[test]
  fn take_scope_of_removes_the_entry() {
    let scopes = ScopeStore::new();
    let node = ReactiveNodeId::Signal(SignalStore::new().alloc_value(0));
    let scope = scopes.alloc_for(node, None);

    assert_eq!(scopes.take_scope_of(node), Some(scope));

    assert!(scopes.node_scopes.borrow().is_empty());
    assert_eq!(scopes.take_scope_of(node), None);
  }
}
