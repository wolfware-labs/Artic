//! The generational arena that stores the UI tree.
//!
//! [`TreeArena`] owns every node as a `Box<dyn Object>`, and [`NodeId`] is the handle used to reach one.
//! Handles are checked against a per-slot generation, so a handle to a freed node stops resolving instead of
//! pointing at whatever node reuses its slot.
//!
//! Every node belongs to two trees at once, each with its own parent and ordered children: the logical tree
//! the markup declares, and the visual tree that is laid out and drawn. Methods that work on links take a
//! [`Tree`] to say which one, such as [`add_child`](TreeArena::add_child) and
//! [`descendants`](TreeArena::descendants).
//!
//! Every node also has dirty flags saying which frame stages it needs redone. A new node starts with
//! layout and rendering dirty, and a change to one of its properties adds the stages the property's key
//! affects; see [`dirty`](TreeArena::dirty).
//!
//! # How this module is organized
//!
//! `TreeArena` is a front over parts with one job each, one file per part:
//!
//! - `objects`: the objects themselves, and the only part that knows whether an id is live.
//! - `hierarchy`: one tree's parent and child links; the arena keeps one per tree.
//! - `dirty`: which frame stages each node needs redone.
//! - `attached`: attached property values, stored per node.
//! - `inheritance`: how inheritable properties follow the logical tree, as functions over the
//!   objects and the logical links.
//!
//! The other parts are indexed by slot, so the arena checks an id in `objects` before it reaches
//! them. The arena also keeps the rules that span parts: a new node gets a place in every part and
//! its properties share its dirty flags; a freed node leaves every part; a logical edit reconnects
//! inheritance; a visual edit marks the parent for layout; a new attached value shares its node's
//! dirty flags. Each file holds the arena's public methods for its part next to the part.

mod attached;
mod dirty;
mod hierarchy;
mod inheritance;
mod objects;

use crate::Object;
use crate::arena::SlotId;
use attached::AttachedValues;
use dirty::DirtyFlags;
use hierarchy::Hierarchy;
use objects::Objects;
use std::any::Any;
use std::fmt::Debug;
use std::rc::Rc;

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
/// use artic_core::Object;
/// use artic_core::property::AnyProperty;
/// use artic_core::tree::TreeArena;
///
/// #[derive(Debug)]
/// struct Label(&'static str);
/// impl Object for Label {
///   fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
/// }
///
/// let mut arena = TreeArena::new();
/// let id = arena.alloc(Box::new(Label("hello")));
///
/// assert_eq!(arena.get_as::<Label>(id).map(|label| label.0), Some("hello"));
///
/// arena.free(id);
/// assert!(arena.get(id).is_none());
/// ```
#[derive(Debug, Default)]
pub struct TreeArena {
  objects: Objects,
  logical: Hierarchy,
  visual: Hierarchy,
  dirty_flags: DirtyFlags,
  attached_values: AttachedValues,
}

/// A handle to a node stored in a [`TreeArena`].
///
/// A `NodeId` pairs a slot index with the generation that slot had when the node was allocated. Lookups
/// compare both, so an id whose node was freed never resolves to whatever node later takes its slot.
///
/// Ids are cheap to copy and compare, and `Option<NodeId>` is the same size as `NodeId`. An id is only
/// meaningful for the arena that returned it.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct NodeId(SlotId);

impl NodeId {
  /// The index of the node's slot, which the parts of the arena are indexed by.
  fn slot(self) -> usize {
    self.0.index()
  }
}

/// Stores `value` at `slot`, growing `values` by one for a slot the arena just appended.
fn put<T>(values: &mut Vec<T>, slot: usize, value: T) {
  if slot == values.len() {
    values.push(value);
  } else {
    values[slot] = value;
  }
}

/// Which of a node's two trees an operation works on.
///
/// Every node in a [`TreeArena`] belongs to two trees at once, with independent parent and child
/// links. The *logical* tree is the structure the markup declares; names, resources and inherited
/// values follow it. The *visual* tree is what gets measured and drawn, including the nodes a
/// control's template adds around its content.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Tree {
  /// The tree the markup declares.
  Logical,
  /// The tree that is laid out and drawn.
  Visual,
}

impl TreeArena {
  /// Creates an empty arena.
  ///
  /// It doesn't allocate until the first node is added.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      objects: Objects::new(),
      logical: Hierarchy::new(),
      visual: Hierarchy::new(),
      dirty_flags: DirtyFlags::new(),
      attached_values: AttachedValues::new(),
    }
  }

  /// Moves `value` into the arena and returns the id that refers to it.
  ///
  /// The most recently freed slot is reused if there is one; otherwise a new slot is appended. The new
  /// node has no parent or children in either tree and starts with layout and rendering dirty. Each
  /// property it lists through [`Object::visit_properties`] is connected to its dirty flags, so a later
  /// change to that property marks the node dirty.
  ///
  /// # Panics
  ///
  /// Panics if the arena would need more than `u32::MAX` slots.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::{Object};
  /// use artic_core::property::AnyProperty;
  /// use artic_core::tree::TreeArena;
  ///
  /// #[derive(Debug)]
  /// struct Label;
  /// impl Object for Label {
  ///   fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  /// }
  ///
  /// let mut arena = TreeArena::new();
  /// let id = arena.alloc(Box::new(Label));
  ///
  /// assert!(arena.get(id).is_some());
  /// assert_eq!(arena.len(), 1);
  /// ```
  #[must_use = "node ID needed to get or free the node from the arena"]
  pub fn alloc(&mut self, value: Box<dyn Object>) -> NodeId {
    let id = self.objects.alloc(value);
    self.logical.insert(id);
    self.visual.insert(id);
    let dirty = self.dirty_flags.reset(id.slot());
    self
      .objects
      .object(id)
      .visit_properties(&mut |property| property.attach(Rc::clone(&dirty)));
    id
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
  /// The node's inheritable properties stop following their ancestors. Its tree links are not touched:
  /// its parent still lists it among its children and its children still name it as their parent.
  /// [`detach`](TreeArena::detach) it first, or use [`free_subtree`](TreeArena::free_subtree), which
  /// detaches and frees a whole subtree.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::{Object};
  /// use artic_core::property::AnyProperty;
  /// use artic_core::tree::TreeArena;
  ///
  /// #[derive(Debug)]
  /// struct Label;
  /// impl Object for Label {
  ///   fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  /// }
  ///
  /// let mut arena = TreeArena::new();
  /// let id = arena.alloc(Box::new(Label));
  ///
  /// assert!(arena.free(id).is_some());
  /// assert!(arena.free(id).is_none()); // already freed
  /// assert!(arena.is_empty());
  /// ```
  pub fn free(&mut self, node_id: NodeId) -> Option<Box<dyn Object>> {
    let object = self.objects.free(node_id)?;
    object.visit_properties(&mut |property| property.inherit_from(None));
    self.logical.remove(node_id);
    self.visual.remove(node_id);
    self.attached_values.remove(node_id.slot());
    Some(object)
  }

  /// Returns a reference to the node referred to by `id`, or `None` if `id` is stale.
  ///
  /// Use [`get_as`](TreeArena::get_as) to get the node as its concrete type.
  #[must_use]
  pub fn get(&self, id: NodeId) -> Option<&dyn Object> {
    self.objects.get(id)
  }

  /// Returns a reference to the node referred to by `id` as a `T`.
  ///
  /// Returns `None` if `id` is stale or if the node isn't a `T`.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::{Object};
  /// use artic_core::property::AnyProperty;
  /// use artic_core::tree::TreeArena;
  ///
  /// #[derive(Debug)]
  /// struct Label(&'static str);
  /// impl Object for Label {
  ///   fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  /// }
  ///
  /// #[derive(Debug)]
  /// struct Button;
  /// impl Object for Button {
  ///   fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  /// }
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
    self.objects.get_mut(id)
  }

  /// Returns a mutable reference to the node referred to by `id` as a `T`.
  ///
  /// Returns `None` if `id` is stale or if the node isn't a `T`.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::{Object};
  /// use artic_core::property::AnyProperty;
  /// use artic_core::tree::TreeArena;
  ///
  /// #[derive(Debug)]
  /// struct Counter(u32);
  /// impl Object for Counter {
  ///   fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  /// }
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
    self.objects.len()
  }

  /// Returns `true` if the arena holds no live nodes.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.objects.is_empty()
  }

  /// Returns `true` if `id` refers to a live node.
  ///
  /// This is `false` once the node has been freed, even if its slot has since been reused by another node.
  /// The answer is only meaningful for ids returned by this arena: an id from another arena can match a live
  /// node here by chance.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::{Object};
  /// use artic_core::property::AnyProperty;
  /// use artic_core::tree::TreeArena;
  ///
  /// #[derive(Debug)]
  /// struct Label;
  /// impl Object for Label {
  ///   fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  /// }
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
    self.objects.contains(id)
  }

  /// Panics if `id` is stale: every edit checks its nodes before it reaches their parts.
  #[track_caller]
  fn check(&self, id: NodeId) {
    self.objects.object(id);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::property::AnyProperty;
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

  impl Object for Control {
    fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  }

  impl Object for OtherControl {
    fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
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
  fn get_as_mut_wrong_type_returns_none() {
    let mut arena = TreeArena::new();
    let n1 = arena.alloc(Box::new(Control));
    assert!(arena.get_as_mut::<OtherControl>(n1).is_none());
  }

  #[derive(Debug, PartialEq, Eq)]
  struct Counter(u32);

  impl Object for Counter {
    fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  }

  #[test]
  fn nodes_of_different_types_share_one_arena() {
    let mut arena = TreeArena::new();
    let control = arena.alloc(Box::new(Control));
    let counter = arena.alloc(Box::new(Counter(7)));
    assert_eq!(arena.len(), 2);
    assert_eq!(arena.get_as::<Control>(control), Some(&Control));
    assert_eq!(arena.get_as::<Counter>(counter), Some(&Counter(7)));
    assert!(arena.get_as::<Counter>(control).is_none());
    assert!(arena.get_as::<Control>(counter).is_none());
  }

  #[test]
  fn get_as_mut_changes_are_visible_through_get_as() {
    let mut arena = TreeArena::new();
    let n1 = arena.alloc(Box::new(Counter(1)));
    arena.get_as_mut::<Counter>(n1).unwrap().0 += 1;
    assert_eq!(arena.get_as::<Counter>(n1), Some(&Counter(2)));
  }

  #[test]
  fn get_and_get_mut_return_the_node_as_object() {
    let mut arena = TreeArena::new();
    let n1 = arena.alloc(Box::new(Counter(3)));
    assert_eq!(format!("{:?}", arena.get(n1).unwrap()), "Counter(3)");
    assert_eq!(format!("{:?}", arena.get_mut(n1).unwrap()), "Counter(3)");
  }

  #[test]
  fn free_returns_the_node_with_its_concrete_type() {
    let mut arena = TreeArena::new();
    let n1 = arena.alloc(Box::new(Counter(5)));
    let node: Box<dyn Any> = arena.free(n1).unwrap();
    assert_eq!(node.downcast_ref::<Counter>(), Some(&Counter(5)));
    assert!(arena.is_empty());
  }

  #[test]
  fn stale_id_returns_none_from_every_accessor() {
    let mut arena = TreeArena::new();
    let n1 = arena.alloc(Box::new(Control));
    arena.free(n1);
    let n2 = arena.alloc(Box::new(Control));
    assert!(arena.get(n1).is_none());
    assert!(arena.get_mut(n1).is_none());
    assert!(arena.get_as::<Control>(n1).is_none());
    assert!(arena.get_as_mut::<Control>(n1).is_none());
    assert!(!arena.contains(n1));
    assert!(arena.free(n1).is_none());
    assert!(arena.contains(n2));
    assert_eq!(arena.len(), 1);
  }

  #[test]
  fn copied_node_ids_are_equal_and_distinct_nodes_are_not() {
    let mut arena = TreeArena::new();
    let n1 = arena.alloc(Box::new(Control));
    let n2 = arena.alloc(Box::new(Control));
    let copy = n1;
    assert_eq!(copy, n1);
    assert_ne!(n1, n2);
  }

  #[test]
  fn default_tree_arena_is_empty() {
    let arena = TreeArena::default();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
  }

  // A new node has no parent in either tree, and a stale id has none either.
  #[test]
  fn new_node_has_no_parent() {
    let mut arena = TreeArena::new();
    let node = arena.alloc(Box::new(Control));

    assert_eq!(arena.parent(Tree::Logical, node), None);
    assert_eq!(arena.parent(Tree::Visual, node), None);
    arena.free(node);
    assert_eq!(arena.parent(Tree::Logical, node), None);
  }
}
