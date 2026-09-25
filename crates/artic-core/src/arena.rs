//! The generational arena that stores the UI tree.
//!
//! [`TreeArena`] owns every node as a `Box<dyn Object>`, and [`NodeId`] is the handle used to reach one.
//! Handles are checked against a per-slot generation, so a handle to a freed node stops resolving instead of
//! pointing at whatever node reuses its slot.

use crate::object::Object;
use std::any::Any;
use std::fmt::Debug;
use std::mem::replace;
use std::num::NonZeroU32;

/// Position of a slot in [`TreeArena::slots`].
type NodeIndex = u32;
/// Counter bumped every time a slot is freed, used to detect stale [`NodeId`]s.
///
/// It is non-zero so that `Option<NodeId>` has the same size as `NodeId`.
type NodeGeneration = NonZeroU32;

/// A generational arena that owns the nodes of the UI tree.
///
/// Every node is stored as a `Box<dyn Object>` in a slot and referenced by a [`NodeId`] handle instead of a
/// pointer or an `Rc<RefCell<_>>`. A handle stays valid until its node is [freed](TreeArena::free); after that,
/// every lookup through it returns `None`, even once the slot has been reused for another node.
///
/// Freed slots are kept in a free list and reused by later allocations, so the backing storage only grows
/// when there is no free slot left.
///
/// # Examples
///
/// ```
/// use artic_core::{Object, TreeArena};
///
/// #[derive(Debug)]
/// struct Label(&'static str);
/// impl Object for Label {}
///
/// let mut arena = TreeArena::new();
/// let id = arena.alloc(Box::new(Label("hello")));
///
/// assert_eq!(arena.get_as::<Label>(id).map(|label| label.0), Some("hello"));
///
/// arena.free(id);
/// assert!(arena.get(id).is_none());
/// ```
#[derive(Debug)]
pub struct TreeArena {
  slots: Vec<Slot>,
  first_free: Option<NodeIndex>,
  occupied_slots: usize,
}

impl Default for TreeArena {
  fn default() -> Self {
    Self::new()
  }
}

/// A handle to a node stored in a [`TreeArena`].
///
/// A `NodeId` pairs a slot index with the generation that slot had when the node was allocated. Lookups
/// compare both, so an id whose node was freed never resolves to whatever node later takes its slot.
///
/// Ids are cheap to copy and compare, and `Option<NodeId>` is the same size as `NodeId`. An id is only
/// meaningful for the arena that returned it.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct NodeId {
  index: NodeIndex,
  generation: NodeGeneration,
}

/// Storage cell of a [`TreeArena`].
#[derive(Debug)]
enum Slot {
  /// An empty slot, linked into the arena's free list through `next_free`.
  Free {
    generation: NodeGeneration,
    next_free: Option<NodeIndex>,
  },
  /// A slot holding a live node.
  Occupied {
    generation: NodeGeneration,
    value: Box<dyn Object>,
  },
}

impl TreeArena {
  /// Creates an empty arena.
  ///
  /// It doesn't allocate until the first node is added.
  pub const fn new() -> Self {
    Self {
      slots: Vec::new(),
      occupied_slots: 0,
      first_free: None,
    }
  }

  /// Moves `value` into the arena and returns the id that refers to it.
  ///
  /// The most recently freed slot is reused if there is one; otherwise a new slot is appended.
  ///
  /// # Panics
  ///
  /// Panics if the arena would need more than `u32::MAX` slots.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::{Object, TreeArena};
  ///
  /// #[derive(Debug)]
  /// struct Label;
  /// impl Object for Label {}
  ///
  /// let mut arena = TreeArena::new();
  /// let id = arena.alloc(Box::new(Label));
  ///
  /// assert!(arena.get(id).is_some());
  /// assert_eq!(arena.len(), 1);
  /// ```
  pub fn alloc(&mut self, value: Box<dyn Object>) -> NodeId {
    self.occupied_slots += 1;

    if let Some(Slot::Free { next_free, generation }) = self.first_free.map(|i| &self.slots[i as usize]) {
      let index = self.first_free.unwrap();
      let generation = *generation;
      self.first_free = *next_free;
      let _ = replace(&mut self.slots[index as usize], Slot::Occupied { generation, value });
      NodeId { index, generation }
    } else {
      let Ok(index) = self.slots.len().try_into() else {
        panic!("The arena capacity is full. There is no space for more slots");
      };
      let generation = NodeGeneration::MIN;
      let new_slot = Slot::Occupied { generation, value };
      self.slots.push(new_slot);
      NodeId { index, generation }
    }
  }

  /// Removes the node referred to by `node_id` and returns it.
  ///
  /// Returns `None`, and leaves the arena untouched, if `node_id` is stale (its node was already freed) or
  /// was never returned by this arena.
  ///
  /// Freeing bumps the slot's generation, which invalidates `node_id` and every copy of it. The generation
  /// wraps around after `u32::MAX - 1` reuses of the same slot, so an id kept across that many reuses
  /// could in principle match a newer node.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::{Object, TreeArena};
  ///
  /// #[derive(Debug)]
  /// struct Label;
  /// impl Object for Label {}
  ///
  /// let mut arena = TreeArena::new();
  /// let id = arena.alloc(Box::new(Label));
  ///
  /// assert!(arena.free(id).is_some());
  /// assert!(arena.free(id).is_none()); // already freed
  /// assert!(arena.is_empty());
  /// ```
  pub fn free(&mut self, node_id: NodeId) -> Option<Box<dyn Object>> {
    let Some(Slot::Occupied { generation, .. }) = self.slots.get(node_id.index as usize) else {
      return None;
    };

    if generation != &node_id.generation {
      return None;
    }

    self.occupied_slots -= 1;

    let freed_slot = Slot::Free {
      generation: generation.checked_add(1).unwrap_or(NodeGeneration::MIN),
      next_free: self.first_free,
    };
    self.first_free = Some(node_id.index);
    let Slot::Occupied { value, .. } = replace(&mut self.slots[node_id.index as usize], freed_slot) else {
      unreachable!();
    };

    Some(value)
  }

  /// Returns a reference to the node referred to by `id`, or `None` if `id` is stale.
  ///
  /// Use [`get_as`](TreeArena::get_as) to get the node as its concrete type.
  pub fn get(&self, id: NodeId) -> Option<&dyn Object> {
    let Slot::Occupied { generation, value } = self.slots.get(id.index as usize)? else {
      return None;
    };

    if generation == &id.generation {
      Some(value.as_ref())
    } else {
      None
    }
  }

  /// Returns a reference to the node referred to by `id` as a `T`.
  ///
  /// Returns `None` if `id` is stale or if the node isn't a `T`.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::{Object, TreeArena};
  ///
  /// #[derive(Debug)]
  /// struct Label(&'static str);
  /// impl Object for Label {}
  ///
  /// #[derive(Debug)]
  /// struct Button;
  /// impl Object for Button {}
  ///
  /// let mut arena = TreeArena::new();
  /// let id = arena.alloc(Box::new(Label("hello")));
  ///
  /// assert_eq!(arena.get_as::<Label>(id).map(|label| label.0), Some("hello"));
  /// assert!(arena.get_as::<Button>(id).is_none()); // wrong type
  /// ```
  pub fn get_as<T>(&self, id: NodeId) -> Option<&T>
  where
    T: Object,
  {
    let node: &dyn Any = self.get(id)?;
    node.downcast_ref()
  }

  /// Returns a mutable reference to the node referred to by `id`, or `None` if `id` is stale.
  ///
  /// Use [`get_as_mut`](TreeArena::get_as_mut) to get the node as its concrete type.
  pub fn get_mut(&mut self, id: NodeId) -> Option<&mut dyn Object> {
    let Slot::Occupied { generation, value } = self.slots.get_mut(id.index as usize)? else {
      return None;
    };

    if generation == &id.generation {
      Some(value.as_mut())
    } else {
      None
    }
  }

  /// Returns a mutable reference to the node referred to by `id` as a `T`.
  ///
  /// Returns `None` if `id` is stale or if the node isn't a `T`.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::{Object, TreeArena};
  ///
  /// #[derive(Debug)]
  /// struct Counter(u32);
  /// impl Object for Counter {}
  ///
  /// let mut arena = TreeArena::new();
  /// let id = arena.alloc(Box::new(Counter(0)));
  ///
  /// if let Some(counter) = arena.get_as_mut::<Counter>(id) {
  ///   counter.0 += 1;
  /// }
  /// assert_eq!(arena.get_as::<Counter>(id).map(|counter| counter.0), Some(1));
  /// ```
  pub fn get_as_mut<T>(&mut self, id: NodeId) -> Option<&mut T>
  where
    T: Object,
  {
    let node: &mut dyn Any = self.get_mut(id)?;
    node.downcast_mut()
  }

  /// Returns the number of live nodes in the arena.
  ///
  /// Freed slots waiting to be reused aren't counted.
  pub fn len(&self) -> usize {
    self.occupied_slots
  }

  /// Returns `true` if the arena holds no live nodes.
  pub fn is_empty(&self) -> bool {
    self.occupied_slots == 0
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::{assert_eq, assert_ne};
  use static_assertions::{assert_eq_size, assert_impl_all, assert_not_impl_any};

  assert_eq_size!(NodeId, Option<NodeId>);
  assert_not_impl_any!(TreeArena: Send);
  assert_impl_all!(NodeId: Copy, Send, Sync);

  #[derive(Debug, PartialEq, Eq)]
  struct Control;

  #[derive(Debug, PartialEq, Eq)]
  struct Control2;

  impl Object for Control {}

  impl Object for Control2 {}

  #[test]
  fn stale_id_after_free_returns_none() {
    let mut arena = TreeArena::new();
    let node_id = arena.alloc(Box::new(Control));
    arena.free(node_id);
    assert!(arena.get(node_id).is_none());
  }

  #[test]
  fn generation_distinguishes_reused_slots() {
    let mut arena = TreeArena::new();
    let mut control: Box<dyn Object> = Box::new(Control);
    let first_node_id = arena.alloc(control);
    control = arena.free(first_node_id).unwrap();
    let second_node_id = arena.alloc(control);
    assert!(arena.get(first_node_id).is_none());
    assert_eq!(arena.get_as::<Control>(second_node_id), Some(&Control));
    assert_eq!(first_node_id.index, second_node_id.index);
    assert_ne!(second_node_id.generation, first_node_id.generation);
  }

  #[test]
  fn get_as_wrong_type_returns_none() {
    let mut arena = TreeArena::new();
    let control = Box::new(Control);
    let node_id = arena.alloc(control);
    assert_eq!(arena.get_as::<Control>(node_id), Some(&Control));
    assert!(arena.get_as::<Control2>(node_id).is_none())
  }

  #[test]
  fn len_returns_to_zero_after_freeing_all_nodes() {
    let mut arena = TreeArena::new();
    let control = Box::new(Control);
    let node_id = arena.alloc(control);
    arena.free(node_id).unwrap();
    assert_eq!(arena.len(), 0);
  }
}
