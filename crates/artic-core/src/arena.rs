use std::fmt::Debug;
use std::mem::replace;
use std::num::NonZeroU32;

type SlotIndex = u32;
type SlotGeneration = NonZeroU32;

#[derive(Debug)]
pub struct Arena<TValue> {
  slots: Vec<Slot<TValue>>,
  first_free: Option<SlotIndex>,
  occupied_slots: usize,
}

/// Storage cell of an [`Arena`].
#[derive(Debug)]
enum Slot<TValue> {
  /// An empty slot, linked into the arena's free list through `next_free`.
  Free {
    generation: SlotGeneration,
    next_free: Option<SlotIndex>,
  },
  /// A slot holding a live value.
  Occupied { generation: SlotGeneration, value: TValue },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct SlotId {
  index: SlotIndex,
  generation: SlotGeneration,
}

impl<TValue> Default for Arena<TValue> {
  fn default() -> Self {
    Self::new()
  }
}

impl<TValue> Arena<TValue> {
  #[must_use]
  pub const fn new() -> Self {
    Self {
      slots: Vec::new(),
      occupied_slots: 0,
      first_free: None,
    }
  }

  #[must_use = "slot ID needed to get or free the value from the arena"]
  pub fn alloc(&mut self, value: TValue) -> SlotId {
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
    SlotId { index, generation }
  }

  pub fn free(&mut self, id: SlotId) -> Option<TValue> {
    let slot = self.slots.get_mut(id.index as usize)?;

    let Slot::Occupied { generation, .. } = &slot else {
      return None;
    };

    if *generation != id.generation {
      return None;
    }

    self.occupied_slots -= 1;

    let old_slot = replace(
      slot,
      Slot::Free {
        generation: generation.checked_add(1).unwrap_or(SlotGeneration::MIN),
        next_free: self.first_free,
      },
    );

    let Slot::Occupied { value, .. } = old_slot else {
      unreachable!("checked above that the slot is occupied");
    };

    self.first_free = Some(id.index);

    Some(value)
  }

  pub fn get(&self, id: SlotId) -> Option<&TValue> {
    let Slot::Occupied { generation, value } = self.slots.get(id.index as usize)? else {
      return None;
    };

    if generation == &id.generation { Some(value) } else { None }
  }

  pub fn get_mut(&mut self, id: SlotId) -> Option<&mut TValue> {
    let Slot::Occupied { generation, value } = self.slots.get_mut(id.index as usize)? else {
      return None;
    };

    if generation == &id.generation { Some(value) } else { None }
  }

  pub fn len(&self) -> usize {
    self.occupied_slots
  }

  pub fn is_empty(&self) -> bool {
    self.occupied_slots == 0
  }

  pub fn contains(&self, id: SlotId) -> bool {
    self.get(id).is_some()
  }

  fn alloc_new_slot(&mut self, value: TValue) -> SlotId {
    let Ok(index) = self.slots.len().try_into() else {
      panic!("the arena capacity is full");
    };
    let generation = SlotGeneration::MIN;
    let new_slot = Slot::Occupied { generation, value };
    self.slots.push(new_slot);
    self.occupied_slots += 1;
    SlotId { index, generation }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::{assert_eq, assert_ne};
  use static_assertions::{assert_eq_size, assert_impl_all};

  assert_eq_size!(SlotId, Option<SlotId>);
  assert_eq_size!(SlotId, u64);
  assert_impl_all!(SlotId: Copy, Send, Sync);

  #[derive(Debug, PartialEq, Eq)]
  struct Item;

  #[test]
  fn stale_id_after_free_returns_none() {
    let mut arena = Arena::new();
    let n1 = arena.alloc(Item);
    arena.free(n1);
    assert!(arena.get(n1).is_none());
  }

  #[test]
  fn generation_distinguishes_reused_slots() {
    let mut arena = Arena::new();
    let mut control = Item;
    let n1 = arena.alloc(control);
    control = arena.free(n1).unwrap();
    let n2 = arena.alloc(control);
    assert!(arena.get(n1).is_none());
    assert_eq!(arena.get(n2), Some(&Item));
    assert_eq!(n1.index, n2.index);
    assert_ne!(n2.generation, n1.generation);
  }

  #[test]
  fn stale_id_does_not_resolve_after_slot_reuse() {
    let mut arena = Arena::new();
    let n1 = arena.alloc(Item);
    arena.free(n1).unwrap();
    let n2 = arena.alloc(Item);
    assert!(arena.get(n1).is_none());
    assert_eq!(n2.index, n1.index);
  }

  #[test]
  fn len_tracks_allocs_and_frees() {
    let mut arena = Arena::new();
    assert_eq!(arena.len(), 0);
    let n1 = arena.alloc(Item);
    assert_eq!(arena.len(), 1);
    let n2 = arena.alloc(Item);
    assert_eq!(arena.len(), 2);
    assert!(arena.free(n1).is_some());
    assert_eq!(arena.len(), 1);
    assert!(arena.free(n2).is_some());
    assert_eq!(arena.len(), 0);
  }

  #[test]
  fn double_free_returns_none_and_keeps_len() {
    let mut arena = Arena::new();
    let n1 = arena.alloc(Item);
    let _ = arena.alloc(Item);
    assert!(arena.free(n1).is_some());
    assert!(arena.free(n1).is_none());
    assert_eq!(arena.len(), 1);
  }

  #[test]
  fn generation_wraps_from_max_to_min() {
    let mut arena = Arena::new();
    let mut n1 = arena.alloc(Item);
    n1.generation = SlotGeneration::MAX;
    if let Slot::Occupied { generation, .. } = &mut arena.slots[n1.index as usize] {
      *generation = SlotGeneration::MAX;
    } else {
      panic!("test setup: the new node's slot should be occupied");
    }
    assert!(arena.free(n1).is_some());
    let n2 = arena.alloc(Item);
    assert_eq!(n2.index, n1.index);
    assert_eq!(n2.generation, SlotGeneration::MIN);
  }

  #[test]
  fn out_of_range_id_returns_none() {
    let mut arena = Arena::new();
    let mut n1 = arena.alloc(Item);
    n1.index = 1;

    assert!(arena.free(n1).is_none());
    assert!(arena.get(n1).is_none());
    assert!(arena.get_mut(n1).is_none());
  }

  #[test]
  fn len_is_correct_after_slot_reuse() {
    let mut arena = Arena::new();
    let n1 = arena.alloc(Item);
    let _ = arena.alloc(Item);
    arena.free(n1);
    let _ = arena.alloc(Item);
    assert_eq!(arena.len(), 2);
  }

  #[test]
  fn is_not_empty_after_alloc() {
    let mut arena = Arena::new();
    let _ = arena.alloc(Item);
    assert!(!arena.is_empty());
  }
}
