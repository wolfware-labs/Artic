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
  #[must_use]
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
  #[must_use = "node ID needed to get or free the node from the arena"]
  pub fn alloc(&mut self, value: Box<dyn Object>) -> NodeId {
    let Some(index) = self.first_free else {
      return self.alloc_new_slot(value);
    };

    let Some(slot) = self.slots.get_mut(index as usize) else {
      panic!("slot free index out of bounds");
    };

    let Slot::Free { next_free, generation } = slot else {
      panic!("slot free index does not belong to a free slot");
    };

    let generation = *generation;
    self.first_free = *next_free;
    *slot = Slot::Occupied { generation, value };
    self.occupied_slots += 1;
    NodeId { index, generation }
  }

  fn alloc_new_slot(&mut self, value: Box<dyn Object>) -> NodeId {
    let Ok(index) = self.slots.len().try_into() else {
      panic!("the arena capacity is full");
    };
    let generation = NodeGeneration::MIN;
    let new_slot = Slot::Occupied { generation, value };
    self.slots.push(new_slot);
    self.occupied_slots += 1;
    NodeId { index, generation }
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
    let slot = self.slots.get_mut(node_id.index as usize)?;

    let Slot::Occupied { generation, .. } = &slot else {
      return None;
    };

    if *generation != node_id.generation {
      return None;
    }

    self.occupied_slots -= 1;

    let old_slot = replace(
      slot,
      Slot::Free {
        generation: generation.checked_add(1).unwrap_or(NodeGeneration::MIN),
        next_free: self.first_free,
      },
    );

    let Slot::Occupied { value, .. } = old_slot else {
      unreachable!("checked above that the slot is occupied");
    };

    self.first_free = Some(node_id.index);

    Some(value)
  }

  /// Returns a reference to the node referred to by `id`, or `None` if `id` is stale.
  ///
  /// Use [`get_as`](TreeArena::get_as) to get the node as its concrete type.
  #[must_use]
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
  #[must_use]
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
  #[must_use]
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
  #[must_use]
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
  #[must_use]
  pub fn len(&self) -> usize {
    self.occupied_slots
  }

  /// Returns `true` if the arena holds no live nodes.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.occupied_slots == 0
  }

  /// Returns `true` if `id` refers to a live node in this arena.
  ///
  /// This is `false` once the node has been freed, even if its slot has since been reused by another node.
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
  /// assert!(arena.contains(id));
  ///
  /// arena.free(id);
  /// let reused = arena.alloc(Box::new(Label)); // takes the freed slot
  /// assert!(!arena.contains(id));
  /// assert!(arena.contains(reused));
  /// ```
  #[must_use]
  pub fn contains(&self, id: NodeId) -> bool {
    self.get(id).is_some()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::{assert_eq, assert_ne};
  use static_assertions::{assert_eq_size, assert_impl_all, assert_not_impl_any};

  assert_eq_size!(NodeId, Option<NodeId>);
  assert_eq_size!(NodeId, u64);
  assert_not_impl_any!(TreeArena: Send);
  assert_impl_all!(NodeId: Copy, Send, Sync);

  #[derive(Debug, PartialEq, Eq)]
  struct Control;

  #[derive(Debug, PartialEq, Eq)]
  struct OtherControl;

  impl Object for Control {}

  impl Object for OtherControl {}

  #[test]
  fn stale_id_after_free_returns_none() {
    let mut arena = TreeArena::new();
    let n1 = arena.alloc(Box::new(Control));
    arena.free(n1);
    assert!(arena.get(n1).is_none());
  }

  #[test]
  fn generation_distinguishes_reused_slots() {
    let mut arena = TreeArena::new();
    let mut control: Box<dyn Object> = Box::new(Control);
    let n1 = arena.alloc(control);
    control = arena.free(n1).unwrap();
    let n2 = arena.alloc(control);
    assert!(arena.get(n1).is_none());
    assert_eq!(arena.get_as::<Control>(n2), Some(&Control));
    assert_eq!(n1.index, n2.index);
    assert_ne!(n2.generation, n1.generation);
  }

  #[test]
  fn get_as_wrong_type_returns_none() {
    let mut arena = TreeArena::new();
    let control = Box::new(Control);
    let n1 = arena.alloc(control);
    assert_eq!(arena.get_as::<Control>(n1), Some(&Control));
    assert!(arena.get_as::<OtherControl>(n1).is_none());
  }

  #[test]
  fn stale_id_does_not_resolve_after_slot_reuse() {
    let mut arena = TreeArena::new();
    let n1 = arena.alloc(Box::new(Control));
    arena.free(n1).unwrap();
    let n2 = arena.alloc(Box::new(Control));
    assert!(arena.get(n1).is_none());
    assert_eq!(n2.index, n1.index);
  }

  #[test]
  fn len_tracks_allocs_and_frees() {
    let mut arena = TreeArena::new();
    assert_eq!(arena.len(), 0);
    let n1 = arena.alloc(Box::new(Control));
    assert_eq!(arena.len(), 1);
    let n2 = arena.alloc(Box::new(Control));
    assert_eq!(arena.len(), 2);
    assert!(arena.free(n1).is_some());
    assert_eq!(arena.len(), 1);
    assert!(arena.free(n2).is_some());
    assert_eq!(arena.len(), 0);
  }

  #[test]
  fn double_free_returns_none_and_keeps_len() {
    let mut arena = TreeArena::new();
    let n1 = arena.alloc(Box::new(Control));
    let _ = arena.alloc(Box::new(Control));
    assert!(arena.free(n1).is_some());
    assert!(arena.free(n1).is_none());
    assert_eq!(arena.len(), 1);
  }

  #[test]
  fn generation_wraps_from_max_to_min() {
    let mut arena = TreeArena::new();
    let mut n1 = arena.alloc(Box::new(Control));
    n1.generation = NodeGeneration::MAX;
    if let Slot::Occupied { generation, .. } = &mut arena.slots[n1.index as usize] {
      *generation = NodeGeneration::MAX;
    } else {
      unreachable!();
    }
    assert!(arena.free(n1).is_some());
    let n2 = arena.alloc(Box::new(Control));
    assert_eq!(n2.index, n1.index);
    assert_eq!(n2.generation, NodeGeneration::MIN);
  }

  #[test]
  fn out_of_range_id_returns_none() {
    let mut arena = TreeArena::new();
    let mut n1 = arena.alloc(Box::new(Control));
    n1.index = 1;
    assert!(arena.free(n1).is_none());
    assert!(arena.get(n1).is_none());
    assert!(arena.get_as::<Control>(n1).is_none());
    assert!(arena.get_mut(n1).is_none());
    assert!(arena.get_as_mut::<Control>(n1).is_none());
  }

  #[test]
  fn get_as_mut_wrong_type_returns_none() {
    let mut arena = TreeArena::new();
    let n1 = arena.alloc(Box::new(Control));
    assert!(arena.get_as_mut::<OtherControl>(n1).is_none());
  }

  #[test]
  fn len_is_correct_after_slot_reuse() {
    let mut arena = TreeArena::new();
    let n1 = arena.alloc(Box::new(Control));
    let _ = arena.alloc(Box::new(Control));
    arena.free(n1);
    let _ = arena.alloc(Box::new(Control));
    assert_eq!(arena.len(), 2);
  }

  #[test]
  fn is_not_empty_after_alloc() {
    let mut arena = TreeArena::new();
    let _ = arena.alloc(Box::new(Control));
    assert!(!arena.is_empty());
  }
}
