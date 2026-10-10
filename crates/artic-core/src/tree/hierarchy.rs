//! The structure of the two trees: who is whose parent and child.
//!
//! Every node is in the logical tree and the visual tree at once, with a parent and ordered children
//! in each. A [`Tree`] holds one tree's links, and the UI tree keeps one per kind. This file adds,
//! inserts, moves, detaches and frees nodes, and walks children, ancestors and descendants, always
//! for the [`TreeKind`] it's given.
//!
//! All edits go through two private functions, `attach` and `unlink`, so the rules hold for every
//! public one: an edit can't make a cycle or reach a freed node, an edit in the logical tree
//! reconnects inherited properties (see `inheritance`), and an edit in the visual tree marks the
//! parent for layout (see `dirty`), since its children decide its size.

use crate::property::Affects;
use crate::tree::{NodeId, TreeKind, UiTree, inheritance, put};
use std::collections::HashSet;
use std::iter::{from_fn, successors};

const TREES: [TreeKind; 2] = [TreeKind::Logical, TreeKind::Visual];

/// One tree's links: each live node's parent and ordered children.
///
/// An entry remembers its node, so a stale id finds nothing, even once its slot is reused.
#[derive(Debug, Default)]
pub(super) struct Tree {
  entries: Vec<Option<Entry>>,
}

#[derive(Debug)]
struct Entry {
  node: NodeId,
  parent: Option<NodeId>,
  children: Vec<NodeId>,
}

impl Tree {
  pub(super) const fn new() -> Self {
    Self { entries: Vec::new() }
  }

  /// Gives `node` an entry with no parent and no children.
  pub(super) fn insert(&mut self, node: NodeId) {
    let entry = Entry {
      node,
      parent: None,
      children: Vec::new(),
    };
    put(&mut self.entries, node.slot(), Some(entry));
  }

  /// Drops `node`'s entry. The nodes it's linked to keep their links to it.
  pub(super) fn remove(&mut self, node: NodeId) {
    self.entries[node.slot()] = None;
  }

  pub(super) fn parent(&self, node: NodeId) -> Option<NodeId> {
    self.entry(node)?.parent
  }

  pub(super) fn children(&self, node: NodeId) -> &[NodeId] {
    self.entry(node).map(|entry| entry.children.as_slice()).unwrap_or_default()
  }

  pub(super) fn ancestors(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    successors(self.parent(node), move |ancestor| self.parent(*ancestor))
  }

  pub(super) fn descendants(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    let mut stack: Vec<NodeId> = self.children(node).iter().rev().copied().collect();
    from_fn(move || {
      let next = stack.pop()?;
      stack.extend(self.children(next).iter().rev());
      Some(next)
    })
  }

  /// Whether making `child` a child of `parent` would close a loop.
  pub(super) fn would_cycle(&self, parent: NodeId, child: NodeId) -> bool {
    child == parent || self.ancestors(parent).any(|ancestor| ancestor == child)
  }

  /// Makes `child`, which has no parent, the child of `parent` at `index`.
  #[track_caller]
  pub(super) fn link(&mut self, parent: NodeId, index: usize, child: NodeId) {
    self.entry_mut(parent).children.insert(index, child);
    self.entry_mut(child).parent = Some(parent);
  }

  /// Cuts `node` from its parent and returns the parent, or `None` if it has none.
  #[track_caller]
  pub(super) fn unlink(&mut self, node: NodeId) -> Option<NodeId> {
    let parent = self.entry_mut(node).parent.take()?;
    self.entry_mut(parent).children.retain(|child| *child != node);
    Some(parent)
  }

  fn entry(&self, node: NodeId) -> Option<&Entry> {
    self.entries.get(node.slot())?.as_ref().filter(|entry| entry.node == node)
  }

  #[track_caller]
  fn entry_mut(&mut self, node: NodeId) -> &mut Entry {
    let entry = self.entries.get_mut(node.slot()).and_then(Option::as_mut);
    match entry.filter(|entry| entry.node == node) {
      Some(entry) => entry,
      None => panic!("node {node:?} is not in the arena"),
    }
  }
}

impl UiTree {
  /// Returns the parent of `id` in `tree`, or `None` if it has none or `id` is stale.
  #[must_use]
  pub fn parent(&self, tree: TreeKind, id: NodeId) -> Option<NodeId> {
    self.tree(tree).parent(id)
  }

  /// Makes `child` the last child of `parent` in `tree`.
  ///
  /// If `child` already has a parent in `tree`, it leaves that parent first, so a node is never in
  /// two child lists of the same tree. Its links in the other tree don't change.
  ///
  /// # Panics
  ///
  /// Panics if `parent` or `child` is stale, or if `child` is `parent` or one of its ancestors in
  /// `tree`, because the tree would then contain a cycle.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::Object;
  /// use artic_core::property::AnyProperty;
  /// use artic_core::tree::{TreeKind, UiTree};
  ///
  /// #[derive(Debug)]
  /// struct Panel;
  /// impl Object for Panel {
  ///   fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  /// }
  ///
  /// let mut tree = UiTree::new();
  /// let panel = tree.alloc(Box::new(Panel));
  /// let first = tree.alloc(Box::new(Panel));
  /// let second = tree.alloc(Box::new(Panel));
  ///
  /// tree.add_child(TreeKind::Logical, panel, first);
  /// tree.add_child(TreeKind::Logical, panel, second);
  /// assert_eq!(tree.children(TreeKind::Logical, panel).collect::<Vec<_>>(), [first, second]);
  /// assert_eq!(tree.parent(TreeKind::Logical, first), Some(panel));
  /// assert_eq!(tree.parent(TreeKind::Visual, first), None);
  /// ```
  #[track_caller]
  pub fn add_child(&mut self, tree: TreeKind, parent: NodeId, child: NodeId) {
    self.check_attach(tree, parent, child);
    self.unlink(tree, child);
    let index = self.tree(tree).children(parent).len();
    self.attach(tree, parent, index, child);
  }

  /// Makes `child` the child of `parent` at `index` in `tree`, shifting later children right.
  ///
  /// Like [`add_child`](UiTree::add_child), `child` first leaves its current parent in `tree`.
  /// `index` counts the children after that, so moving a node within one parent's list works the
  /// same as inserting a new one.
  ///
  /// # Panics
  ///
  /// Panics in the same cases as [`add_child`](UiTree::add_child), and if `index` is greater than
  /// the number of children.
  #[track_caller]
  pub fn insert_child(&mut self, tree: TreeKind, parent: NodeId, index: usize, child: NodeId) {
    self.check_attach(tree, parent, child);
    let mut len = self.tree(tree).children(parent).len();
    if self.tree(tree).parent(child) == Some(parent) {
      len -= 1;
    }
    assert!(index <= len, "child index {index} is out of range for {len} children");
    self.unlink(tree, child);
    self.attach(tree, parent, index, child);
  }

  /// Removes `node` from its parent's children in `tree` and returns that parent.
  ///
  /// Returns `None`, and changes nothing, if `node` has no parent in `tree`. The node stays in the
  /// tree with its own children; only the link to its parent is cut. In the logical tree, the
  /// node and its descendants then inherit only from what is left above them.
  ///
  /// # Panics
  ///
  /// Panics if `node` is stale.
  #[track_caller]
  pub fn detach(&mut self, tree: TreeKind, node: NodeId) -> Option<NodeId> {
    let parent = self.unlink(tree, node)?;
    if tree == TreeKind::Logical {
      inheritance::resolve(&self.objects, &self.logical, node);
    }
    Some(parent)
  }

  /// Returns the children of `id` in `tree`, in order.
  ///
  /// A stale `id` has no children.
  pub fn children(&self, tree: TreeKind, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    self.tree(tree).children(id).iter().copied()
  }

  /// Returns the ancestors of `id` in `tree`: its parent, then that node's parent, up to the root.
  ///
  /// `id` itself isn't included, and a stale `id` has no ancestors.
  pub fn ancestors(&self, tree: TreeKind, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    self.tree(tree).ancestors(id)
  }

  /// Returns every descendant of `id` in `tree`, depth-first in pre-order: a node, then its first
  /// child's whole subtree, then its next child's.
  ///
  /// `id` itself isn't included, and a stale `id` has no descendants.
  ///
  /// # Examples
  ///
  /// ```
  /// use artic_core::Object;
  /// use artic_core::property::AnyProperty;
  /// use artic_core::tree::{TreeKind, UiTree};
  ///
  /// #[derive(Debug)]
  /// struct Panel;
  /// impl Object for Panel {
  ///   fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  /// }
  ///
  /// let mut tree = UiTree::new();
  /// let [root, a, a1, b] = [(); 4].map(|_| tree.alloc(Box::new(Panel)));
  /// tree.add_child(TreeKind::Visual, root, a);
  /// tree.add_child(TreeKind::Visual, a, a1);
  /// tree.add_child(TreeKind::Visual, root, b);
  ///
  /// assert_eq!(tree.descendants(TreeKind::Visual, root).collect::<Vec<_>>(), [a, a1, b]);
  /// ```
  pub fn descendants(&self, tree: TreeKind, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    self.tree(tree).descendants(id)
  }

  /// Frees `id` and everything below it in either tree.
  ///
  /// The subtree is every node reachable from `id` through child links of either tree, so template
  /// parts that only exist in the visual tree go too. Any freed node whose parent survives is
  /// detached from that parent first, so no live node keeps the id of a freed child, and every child
  /// of a freed node is freed with it.
  ///
  /// # Panics
  ///
  /// Panics if `id` is stale.
  #[track_caller]
  pub fn free_subtree(&mut self, id: NodeId) {
    self.check(id);
    let mut subtree = HashSet::from([id]);
    let mut pending = vec![id];
    while let Some(node) = pending.pop() {
      for tree in TREES {
        for child in self.tree(tree).children(node) {
          if subtree.insert(*child) {
            pending.push(*child);
          }
        }
      }
    }
    for node in &subtree {
      for tree in TREES {
        if self.parent(tree, *node).is_some_and(|parent| !subtree.contains(&parent)) {
          self.unlink(tree, *node);
        }
      }
    }
    for node in subtree {
      self.free(node);
    }
  }

  fn tree(&self, tree: TreeKind) -> &Tree {
    match tree {
      TreeKind::Logical => &self.logical,
      TreeKind::Visual => &self.visual,
    }
  }

  fn tree_mut(&mut self, tree: TreeKind) -> &mut Tree {
    match tree {
      TreeKind::Logical => &mut self.logical,
      TreeKind::Visual => &mut self.visual,
    }
  }

  #[track_caller]
  fn check_attach(&self, tree: TreeKind, parent: NodeId, child: NodeId) {
    self.check(parent);
    self.check(child);
    let is_cycle = self.tree(tree).would_cycle(parent, child);
    assert!(
      !is_cycle,
      "adding node {child:?} under {parent:?} would create a cycle in the {tree:?} tree"
    );
  }

  fn attach(&mut self, tree: TreeKind, parent: NodeId, index: usize, child: NodeId) {
    self.tree_mut(tree).link(parent, index, child);
    self.children_changed(tree, parent);
    if tree == TreeKind::Logical {
      inheritance::resolve(&self.objects, &self.logical, child);
    }
  }

  #[track_caller]
  fn unlink(&mut self, tree: TreeKind, node: NodeId) -> Option<NodeId> {
    let parent = self.tree_mut(tree).unlink(node)?;
    self.children_changed(tree, parent);
    Some(parent)
  }

  /// A node's visual children size and place it, so changing them means laying it out again.
  fn children_changed(&mut self, tree: TreeKind, parent: NodeId) {
    if tree == TreeKind::Visual {
      self.mark_dirty(parent, Affects::LAYOUT);
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::Object;
  use crate::property::{Affects, AnyProperty};
  use crate::tree::{NodeId, TreeKind, UiTree};
  use pretty_assertions::assert_eq;
  use std::panic::{AssertUnwindSafe, catch_unwind};

  #[derive(Debug)]
  struct Control;

  impl Object for Control {
    fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  }

  /// Clears every node's dirty flags.
  fn clean(tree: &mut UiTree, ids: &[NodeId]) {
    for id in ids {
      tree.clear_dirty(*id, Affects::LAYOUT | Affects::RENDER);
    }
  }

  /// Allocates `count` nodes and returns their ids, in order.
  fn nodes(tree: &mut UiTree, count: usize) -> Vec<NodeId> {
    (0..count).map(|_| tree.alloc(Box::new(Control))).collect()
  }

  /// Collects the children of `id` in the `kind` tree.
  fn children(tree: &UiTree, kind: TreeKind, id: NodeId) -> Vec<NodeId> {
    tree.children(kind, id).collect()
  }

  // A node's logical and visual parents are set separately: a template part sits between a control
  // and its content in the visual tree only.
  #[test]
  fn logical_and_visual_parents_are_independent() {
    let mut tree = UiTree::new();
    let [button, border, text] = nodes(&mut tree, 3)[..] else {
      unreachable!()
    };

    tree.add_child(TreeKind::Logical, button, text);
    tree.add_child(TreeKind::Visual, button, border);
    tree.add_child(TreeKind::Visual, border, text);

    assert_eq!(tree.parent(TreeKind::Logical, text), Some(button));
    assert_eq!(tree.parent(TreeKind::Visual, text), Some(border));
    assert_eq!(tree.parent(TreeKind::Logical, border), None);
    assert_eq!(children(&tree, TreeKind::Logical, button), [text]);
    assert_eq!(children(&tree, TreeKind::Visual, button), [border]);
  }

  // Adding a node to a new parent removes it from its old parent's children in the same tree.
  #[test]
  fn reparent_removes_from_old_parent() {
    let mut tree = UiTree::new();
    let [old_parent, new_parent, child] = nodes(&mut tree, 3)[..] else {
      unreachable!()
    };
    tree.add_child(TreeKind::Logical, old_parent, child);

    tree.add_child(TreeKind::Logical, new_parent, child);

    assert_eq!(children(&tree, TreeKind::Logical, old_parent), []);
    assert_eq!(children(&tree, TreeKind::Logical, new_parent), [child]);
    assert_eq!(tree.parent(TreeKind::Logical, child), Some(new_parent));
  }

  // `insert_child` places a child at its index, and moving a node within the same parent counts the
  // index after it left its old place.
  #[test]
  fn insert_child_places_the_child_at_its_index() {
    let mut tree = UiTree::new();
    let [parent, a, b, c] = nodes(&mut tree, 4)[..] else {
      unreachable!()
    };
    tree.add_child(TreeKind::Visual, parent, a);
    tree.add_child(TreeKind::Visual, parent, b);

    tree.insert_child(TreeKind::Visual, parent, 1, c);
    assert_eq!(children(&tree, TreeKind::Visual, parent), [a, c, b]);

    tree.insert_child(TreeKind::Visual, parent, 0, b);
    assert_eq!(children(&tree, TreeKind::Visual, parent), [b, a, c]);
  }

  // `detach` cuts a node from its parent and returns the parent; a node without a parent gives
  // `None`.
  #[test]
  fn detach_returns_the_former_parent() {
    let mut tree = UiTree::new();
    let [parent, child] = nodes(&mut tree, 2)[..] else {
      unreachable!()
    };
    tree.add_child(TreeKind::Logical, parent, child);

    assert_eq!(tree.detach(TreeKind::Logical, child), Some(parent));
    assert_eq!(tree.detach(TreeKind::Logical, child), None);
    assert_eq!(children(&tree, TreeKind::Logical, parent), []);
  }

  // A stale id has no children.
  #[test]
  fn stale_node_has_no_children() {
    let mut tree = UiTree::new();
    let [parent, child] = nodes(&mut tree, 2)[..] else {
      unreachable!()
    };
    tree.add_child(TreeKind::Logical, parent, child);

    tree.free(parent);

    assert_eq!(children(&tree, TreeKind::Logical, parent), []);
  }

  // A stale id doesn't reach the links of the node that reuses its slot: it has no children, and a
  // walk up from a child it left behind stops at it.
  #[test]
  fn stale_id_does_not_reach_the_node_in_its_slot() {
    let mut tree = UiTree::new();
    let [root, parent, child, other] = nodes(&mut tree, 4)[..] else {
      unreachable!()
    };
    tree.add_child(TreeKind::Logical, parent, child);
    tree.free(parent);
    let reused = tree.alloc(Box::new(Control));
    tree.add_child(TreeKind::Logical, root, reused);
    tree.add_child(TreeKind::Logical, reused, other);

    assert_eq!(children(&tree, TreeKind::Logical, parent), []);
    assert_eq!(tree.ancestors(TreeKind::Logical, child).collect::<Vec<_>>(), [parent]);
  }

  // Adding a stale node as a child panics.
  #[test]
  #[should_panic(expected = "is not in the arena")]
  fn adding_a_stale_child_panics() {
    let mut tree = UiTree::new();
    let [parent, child] = nodes(&mut tree, 2)[..] else {
      unreachable!()
    };
    tree.free(child);

    tree.add_child(TreeKind::Logical, parent, child);
  }

  // Inserting past the end of the child list panics.
  #[test]
  #[should_panic(expected = "child index 1 is out of range for 0 children")]
  fn insert_past_the_end_panics() {
    let mut tree = UiTree::new();
    let [parent, child] = nodes(&mut tree, 2)[..] else {
      unreachable!()
    };

    tree.insert_child(TreeKind::Logical, parent, 1, child);
  }

  // A failed edit changes nothing: the child stays under its old parent when the new parent is stale
  // or the index is out of range.
  #[test]
  fn failed_edit_leaves_the_tree_unchanged() {
    let mut tree = UiTree::new();
    let [parent, child, stale] = nodes(&mut tree, 3)[..] else {
      unreachable!()
    };
    tree.add_child(TreeKind::Logical, parent, child);
    tree.free(stale);

    let stale_parent = catch_unwind(AssertUnwindSafe(|| tree.add_child(TreeKind::Logical, stale, child)));
    let past_the_end = catch_unwind(AssertUnwindSafe(|| tree.insert_child(TreeKind::Logical, parent, 2, child)));

    assert!(stale_parent.is_err() && past_the_end.is_err());
    assert_eq!(tree.parent(TreeKind::Logical, child), Some(parent));
    assert_eq!(children(&tree, TreeKind::Logical, parent), [child]);
  }

  // `ancestors` walks from the parent up to the root, in one tree only.
  #[test]
  fn ancestors_walk_up_to_the_root() {
    let mut tree = UiTree::new();
    let [root, middle, leaf] = nodes(&mut tree, 3)[..] else {
      unreachable!()
    };
    tree.add_child(TreeKind::Logical, root, middle);
    tree.add_child(TreeKind::Logical, middle, leaf);

    assert_eq!(tree.ancestors(TreeKind::Logical, leaf).collect::<Vec<_>>(), [middle, root]);
    assert_eq!(tree.ancestors(TreeKind::Visual, leaf).count(), 0);
  }

  // Adding a node under itself or under one of its descendants is rejected, and the tree is left as
  // it was.
  #[test]
  fn cycle_insert_is_rejected() {
    let mut tree = UiTree::new();
    let [root, middle, leaf] = nodes(&mut tree, 3)[..] else {
      unreachable!()
    };
    tree.add_child(TreeKind::Logical, root, middle);
    tree.add_child(TreeKind::Logical, middle, leaf);

    let under_itself = catch_unwind(AssertUnwindSafe(|| tree.add_child(TreeKind::Logical, middle, middle)));
    let under_descendant = catch_unwind(AssertUnwindSafe(|| tree.insert_child(TreeKind::Logical, leaf, 0, root)));

    assert!(under_itself.is_err() && under_descendant.is_err());
    assert_eq!(tree.parent(TreeKind::Logical, middle), Some(root));
    assert_eq!(tree.parent(TreeKind::Logical, root), None);
    assert_eq!(children(&tree, TreeKind::Logical, leaf), []);
  }

  // The cycle check is per tree: a logical ancestor can be a visual child.
  #[test]
  fn cycle_check_looks_at_one_tree_only() {
    let mut tree = UiTree::new();
    let [root, leaf] = nodes(&mut tree, 2)[..] else {
      unreachable!()
    };
    tree.add_child(TreeKind::Logical, root, leaf);

    tree.add_child(TreeKind::Visual, leaf, root);

    assert_eq!(tree.parent(TreeKind::Visual, root), Some(leaf));
  }

  // The cycle panic names both nodes and the tree.
  #[test]
  #[should_panic(expected = "would create a cycle in the Visual tree")]
  fn cycle_panic_names_the_tree() {
    let mut tree = UiTree::new();
    let [node] = nodes(&mut tree, 1)[..] else { unreachable!() };

    tree.add_child(TreeKind::Visual, node, node);
  }

  // `descendants` yields a subtree in pre-order, and each tree follows its own links.
  #[test]
  fn descendants_preorder() {
    let mut tree = UiTree::new();
    let [a, b, c, d, e] = nodes(&mut tree, 5)[..] else {
      unreachable!()
    };
    // Logical: a(b(c, d), e)
    tree.add_child(TreeKind::Logical, a, b);
    tree.add_child(TreeKind::Logical, b, c);
    tree.add_child(TreeKind::Logical, b, d);
    tree.add_child(TreeKind::Logical, a, e);
    // Visual: a(e(d, b(c)))
    tree.add_child(TreeKind::Visual, a, e);
    tree.add_child(TreeKind::Visual, e, d);
    tree.add_child(TreeKind::Visual, e, b);
    tree.add_child(TreeKind::Visual, b, c);

    assert_eq!(tree.descendants(TreeKind::Logical, a).collect::<Vec<_>>(), [b, c, d, e]);
    assert_eq!(tree.descendants(TreeKind::Visual, a).collect::<Vec<_>>(), [e, d, b, c]);
    assert_eq!(tree.descendants(TreeKind::Logical, c).count(), 0);
  }

  // Freeing a control frees its logical content and the visual-only parts of its template, and
  // nothing else.
  #[test]
  fn free_subtree_frees_all_descendants() {
    let mut tree = UiTree::new();
    let [panel, button, border, presenter, text, sibling] = nodes(&mut tree, 6)[..] else {
      unreachable!()
    };
    tree.add_child(TreeKind::Logical, panel, button);
    tree.add_child(TreeKind::Logical, panel, sibling);
    tree.add_child(TreeKind::Logical, button, text);
    tree.add_child(TreeKind::Visual, panel, button);
    tree.add_child(TreeKind::Visual, button, border);
    tree.add_child(TreeKind::Visual, border, presenter);
    tree.add_child(TreeKind::Visual, presenter, text);

    tree.free_subtree(button);

    assert_eq!(tree.len(), 2);
    assert!(tree.contains(panel) && tree.contains(sibling));
    assert_eq!(children(&tree, TreeKind::Logical, panel), [sibling]);
    assert_eq!(children(&tree, TreeKind::Visual, panel), []);
  }

  // A node inside the subtree in one tree but under an outside parent in the other is freed, and the
  // outside parent stops listing it.
  #[test]
  fn free_subtree_detaches_nodes_from_outside_parents() {
    let mut tree = UiTree::new();
    let [owner, popup_root, content] = nodes(&mut tree, 3)[..] else {
      unreachable!()
    };
    tree.add_child(TreeKind::Logical, owner, content);
    tree.add_child(TreeKind::Visual, popup_root, content);

    tree.free_subtree(owner);

    assert!(!tree.contains(content));
    assert_eq!(children(&tree, TreeKind::Visual, popup_root), []);
    assert_eq!(tree.len(), 1);
  }

  // Freeing a root frees the whole tree.
  #[test]
  fn free_subtree_of_a_root_empties_the_arena() {
    let mut tree = UiTree::new();
    let [root, a, b] = nodes(&mut tree, 3)[..] else {
      unreachable!()
    };
    tree.add_child(TreeKind::Logical, root, a);
    tree.add_child(TreeKind::Visual, a, b);

    tree.free_subtree(root);

    assert!(tree.is_empty());
  }

  // Adding, moving or removing a visual child marks the parent for layout; logical edits don't.
  #[test]
  fn visual_edits_mark_the_parent_for_layout() {
    let mut tree = UiTree::new();
    let ids = nodes(&mut tree, 3);
    let [panel, other, child] = ids[..] else { unreachable!() };
    clean(&mut tree, &ids);

    tree.add_child(TreeKind::Logical, panel, child);
    assert_eq!(tree.dirty(panel), Affects::NONE, "logical edits leave layout alone");

    tree.add_child(TreeKind::Visual, panel, child);
    assert_eq!(tree.dirty(panel), Affects::LAYOUT);

    clean(&mut tree, &ids);
    tree.add_child(TreeKind::Visual, other, child);
    assert_eq!(
      (tree.dirty(panel), tree.dirty(other)),
      (Affects::LAYOUT, Affects::LAYOUT),
      "both parents of a move"
    );

    clean(&mut tree, &ids);
    tree.detach(TreeKind::Visual, child);
    assert_eq!(tree.dirty(other), Affects::LAYOUT);
  }
}
