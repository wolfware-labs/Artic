//! Which frame stages each node needs redone.
//!
//! A node's dirty flags are an [`Affects`]: layout, render, or both. A new node starts with both. A
//! property change adds the stages its key affects, through flags the node shares with its
//! properties; an edit to a node's visual children adds layout. The frame stages clear what they
//! redo: layout clears [`Affects::LAYOUT`], rendering clears [`Affects::RENDER`].

use crate::arena::SlotMap;
use crate::property::Affects;
use crate::tree::NodeId;
use std::cell::Cell;
use std::rc::Rc;

/// Each node's dirty flags, shared with its properties.
#[derive(Debug, Default)]
pub(super) struct DirtyFlags {
  flags: SlotMap<Rc<Cell<Affects>>>,
}

impl DirtyFlags {
  pub(super) const fn new() -> Self {
    Self { flags: SlotMap::new() }
  }

  /// Gives `node` new flags with layout and rendering dirty, and returns them for its properties to
  /// share.
  pub(super) fn insert(&mut self, node: NodeId) -> Rc<Cell<Affects>> {
    let flags = Rc::new(Cell::new(Affects::LAYOUT | Affects::RENDER));
    self.flags.insert(node.0, Rc::clone(&flags));
    flags
  }

  /// Drops `node`'s flags. Its properties keep their own reference to them.
  pub(super) fn remove(&mut self, node: NodeId) {
    self.flags.remove(node.0);
  }

  pub(super) fn get(&self, node: NodeId) -> Affects {
    self.flags.get(node.0).map_or(Affects::NONE, |flags| flags.get())
  }

  pub(super) fn mark(&self, node: NodeId, stages: Affects) {
    if let Some(flags) = self.flags.get(node.0) {
      flags.set(flags.get() | stages);
    }
  }

  pub(super) fn clear(&self, node: NodeId, stages: Affects) {
    if let Some(flags) = self.flags.get(node.0) {
      flags.set(flags.get().without(stages));
    }
  }

  pub(super) fn shared(&self, node: NodeId) -> Rc<Cell<Affects>> {
    Rc::clone(self.flags.get(node.0).expect("a live node has dirty flags"))
  }
}
