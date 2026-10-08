use crate::tree::node::TreeNode;
use crate::tree::{NodeId, Tree, TreeArena};
use std::collections::HashSet;
use std::iter::{from_fn, successors};

const TREES: [Tree; 2] = [Tree::Logical, Tree::Visual];

impl TreeArena {
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
  /// use artic_core::tree::{Tree, TreeArena};
  ///
  /// #[derive(Debug)]
  /// struct Panel;
  /// impl Object for Panel {}
  ///
  /// let mut arena = TreeArena::new();
  /// let panel = arena.alloc(Box::new(Panel));
  /// let first = arena.alloc(Box::new(Panel));
  /// let second = arena.alloc(Box::new(Panel));
  ///
  /// arena.add_child(Tree::Logical, panel, first);
  /// arena.add_child(Tree::Logical, panel, second);
  /// assert_eq!(arena.children(Tree::Logical, panel).collect::<Vec<_>>(), [first, second]);
  /// assert_eq!(arena.parent(Tree::Logical, first), Some(panel));
  /// assert_eq!(arena.parent(Tree::Visual, first), None);
  /// ```
  #[track_caller]
  pub fn add_child(&mut self, tree: Tree, parent: NodeId, child: NodeId) {
    self.check_attach(tree, parent, child);
    self.unlink(tree, child);
    let index = self.node(parent).links(tree).children.len();
    self.attach(tree, parent, index, child);
  }

  /// Makes `child` the child of `parent` at `index` in `tree`, shifting later children right.
  ///
  /// Like [`add_child`](TreeArena::add_child), `child` first leaves its current parent in `tree`.
  /// `index` counts the children after that, so moving a node within one parent's list works the
  /// same as inserting a new one.
  ///
  /// # Panics
  ///
  /// Panics in the same cases as [`add_child`](TreeArena::add_child), and if `index` is greater than
  /// the number of children.
  #[track_caller]
  pub fn insert_child(&mut self, tree: Tree, parent: NodeId, index: usize, child: NodeId) {
    self.check_attach(tree, parent, child);
    let mut len = self.node(parent).links(tree).children.len();
    if self.node(child).links(tree).parent == Some(parent) {
      len -= 1;
    }
    assert!(index <= len, "child index {index} is out of range for {len} children");
    self.unlink(tree, child);
    self.attach(tree, parent, index, child);
  }

  /// Removes `node` from its parent's children in `tree` and returns that parent.
  ///
  /// Returns `None`, and changes nothing, if `node` has no parent in `tree`. The node stays in the
  /// arena with its own children; only the link to its parent is cut. In the logical tree, the
  /// node and its descendants then inherit only from what is left above them.
  ///
  /// # Panics
  ///
  /// Panics if `node` is stale.
  #[track_caller]
  pub fn detach(&mut self, tree: Tree, node: NodeId) -> Option<NodeId> {
    let parent = self.unlink(tree, node)?;
    if tree == Tree::Logical {
      self.resolve_inheritance(node);
    }
    Some(parent)
  }

  /// Returns the children of `id` in `tree`, in order.
  ///
  /// A stale `id` has no children.
  pub fn children(&self, tree: Tree, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    self.child_slice(tree, id).iter().copied()
  }

  /// Returns the ancestors of `id` in `tree`: its parent, then that node's parent, up to the root.
  ///
  /// `id` itself isn't included, and a stale `id` has no ancestors.
  pub fn ancestors(&self, tree: Tree, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    successors(self.parent(tree, id), move |ancestor| self.parent(tree, *ancestor))
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
  /// use artic_core::tree::{Tree, TreeArena};
  ///
  /// #[derive(Debug)]
  /// struct Panel;
  /// impl Object for Panel {}
  ///
  /// let mut arena = TreeArena::new();
  /// let [root, a, a1, b] = [(); 4].map(|_| arena.alloc(Box::new(Panel)));
  /// arena.add_child(Tree::Visual, root, a);
  /// arena.add_child(Tree::Visual, a, a1);
  /// arena.add_child(Tree::Visual, root, b);
  ///
  /// assert_eq!(arena.descendants(Tree::Visual, root).collect::<Vec<_>>(), [a, a1, b]);
  /// ```
  pub fn descendants(&self, tree: Tree, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    let mut stack: Vec<NodeId> = self.child_slice(tree, id).iter().rev().copied().collect();
    from_fn(move || {
      let next = stack.pop()?;
      stack.extend(self.child_slice(tree, next).iter().rev());
      Some(next)
    })
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
    self.node(id);
    let mut subtree = HashSet::from([id]);
    let mut pending = vec![id];
    while let Some(node) = pending.pop() {
      for tree in TREES {
        for child in self.child_slice(tree, node) {
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

  fn child_slice(&self, tree: Tree, id: NodeId) -> &[NodeId] {
    self
      .inner
      .get(id.0)
      .map(|node| node.links(tree).children.as_slice())
      .unwrap_or_default()
  }

  #[track_caller]
  fn check_attach(&self, tree: Tree, parent: NodeId, child: NodeId) {
    self.node(parent);
    self.node(child);
    let is_cycle = child == parent || self.ancestors(tree, parent).any(|ancestor| ancestor == child);
    assert!(
      !is_cycle,
      "adding node {child:?} under {parent:?} would create a cycle in the {tree:?} tree"
    );
  }

  fn attach(&mut self, tree: Tree, parent: NodeId, index: usize, child: NodeId) {
    self.node_mut(parent).links_mut(tree).children.insert(index, child);
    self.node_mut(child).links_mut(tree).parent = Some(parent);
    if tree == Tree::Logical {
      self.resolve_inheritance(child);
    }
  }

  #[track_caller]
  fn unlink(&mut self, tree: Tree, node: NodeId) -> Option<NodeId> {
    let parent = self.node_mut(node).links_mut(tree).parent.take()?;
    self.node_mut(parent).links_mut(tree).children.retain(|child| *child != node);
    Some(parent)
  }

  #[track_caller]
  pub(super) fn node(&self, id: NodeId) -> &TreeNode {
    match self.inner.get(id.0) {
      Some(node) => node,
      None => panic!("node {id:?} is not in the arena"),
    }
  }

  #[track_caller]
  pub(super) fn node_mut(&mut self, id: NodeId) -> &mut TreeNode {
    match self.inner.get_mut(id.0) {
      Some(node) => node,
      None => panic!("node {id:?} is not in the arena"),
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::Object;
  use crate::tree::{NodeId, Tree, TreeArena};
  use pretty_assertions::assert_eq;
  use std::panic::{AssertUnwindSafe, catch_unwind};

  #[derive(Debug)]
  struct Control;

  impl Object for Control {}

  /// Allocates `count` nodes and returns their ids, in order.
  fn nodes(arena: &mut TreeArena, count: usize) -> Vec<NodeId> {
    (0..count).map(|_| arena.alloc(Box::new(Control))).collect()
  }

  /// Collects the children of `id` in `tree`.
  fn children(arena: &TreeArena, tree: Tree, id: NodeId) -> Vec<NodeId> {
    arena.children(tree, id).collect()
  }

  // A node's logical and visual parents are set separately: a template part sits between a control
  // and its content in the visual tree only.
  #[test]
  fn logical_and_visual_parents_are_independent() {
    let mut arena = TreeArena::new();
    let [button, border, text] = nodes(&mut arena, 3)[..] else {
      unreachable!()
    };

    arena.add_child(Tree::Logical, button, text);
    arena.add_child(Tree::Visual, button, border);
    arena.add_child(Tree::Visual, border, text);

    assert_eq!(arena.parent(Tree::Logical, text), Some(button));
    assert_eq!(arena.parent(Tree::Visual, text), Some(border));
    assert_eq!(arena.parent(Tree::Logical, border), None);
    assert_eq!(children(&arena, Tree::Logical, button), [text]);
    assert_eq!(children(&arena, Tree::Visual, button), [border]);
  }

  // Adding a node to a new parent removes it from its old parent's children in the same tree.
  #[test]
  fn reparent_removes_from_old_parent() {
    let mut arena = TreeArena::new();
    let [old_parent, new_parent, child] = nodes(&mut arena, 3)[..] else {
      unreachable!()
    };
    arena.add_child(Tree::Logical, old_parent, child);

    arena.add_child(Tree::Logical, new_parent, child);

    assert_eq!(children(&arena, Tree::Logical, old_parent), []);
    assert_eq!(children(&arena, Tree::Logical, new_parent), [child]);
    assert_eq!(arena.parent(Tree::Logical, child), Some(new_parent));
  }

  // `insert_child` places a child at its index, and moving a node within the same parent counts the
  // index after it left its old place.
  #[test]
  fn insert_child_places_the_child_at_its_index() {
    let mut arena = TreeArena::new();
    let [parent, a, b, c] = nodes(&mut arena, 4)[..] else {
      unreachable!()
    };
    arena.add_child(Tree::Visual, parent, a);
    arena.add_child(Tree::Visual, parent, b);

    arena.insert_child(Tree::Visual, parent, 1, c);
    assert_eq!(children(&arena, Tree::Visual, parent), [a, c, b]);

    arena.insert_child(Tree::Visual, parent, 0, b);
    assert_eq!(children(&arena, Tree::Visual, parent), [b, a, c]);
  }

  // `detach` cuts a node from its parent and returns the parent; a node without a parent gives
  // `None`.
  #[test]
  fn detach_returns_the_former_parent() {
    let mut arena = TreeArena::new();
    let [parent, child] = nodes(&mut arena, 2)[..] else {
      unreachable!()
    };
    arena.add_child(Tree::Logical, parent, child);

    assert_eq!(arena.detach(Tree::Logical, child), Some(parent));
    assert_eq!(arena.detach(Tree::Logical, child), None);
    assert_eq!(children(&arena, Tree::Logical, parent), []);
  }

  // A stale id has no children.
  #[test]
  fn stale_node_has_no_children() {
    let mut arena = TreeArena::new();
    let [parent, child] = nodes(&mut arena, 2)[..] else {
      unreachable!()
    };
    arena.add_child(Tree::Logical, parent, child);

    arena.free(parent);

    assert_eq!(children(&arena, Tree::Logical, parent), []);
  }

  // Adding a stale node as a child panics.
  #[test]
  #[should_panic(expected = "is not in the arena")]
  fn adding_a_stale_child_panics() {
    let mut arena = TreeArena::new();
    let [parent, child] = nodes(&mut arena, 2)[..] else {
      unreachable!()
    };
    arena.free(child);

    arena.add_child(Tree::Logical, parent, child);
  }

  // Inserting past the end of the child list panics.
  #[test]
  #[should_panic(expected = "child index 1 is out of range for 0 children")]
  fn insert_past_the_end_panics() {
    let mut arena = TreeArena::new();
    let [parent, child] = nodes(&mut arena, 2)[..] else {
      unreachable!()
    };

    arena.insert_child(Tree::Logical, parent, 1, child);
  }

  // A failed edit changes nothing: the child stays under its old parent when the new parent is stale
  // or the index is out of range.
  #[test]
  fn failed_edit_leaves_the_tree_unchanged() {
    let mut arena = TreeArena::new();
    let [parent, child, stale] = nodes(&mut arena, 3)[..] else {
      unreachable!()
    };
    arena.add_child(Tree::Logical, parent, child);
    arena.free(stale);

    let stale_parent = catch_unwind(AssertUnwindSafe(|| arena.add_child(Tree::Logical, stale, child)));
    let past_the_end = catch_unwind(AssertUnwindSafe(|| arena.insert_child(Tree::Logical, parent, 2, child)));

    assert!(stale_parent.is_err() && past_the_end.is_err());
    assert_eq!(arena.parent(Tree::Logical, child), Some(parent));
    assert_eq!(children(&arena, Tree::Logical, parent), [child]);
  }

  // `ancestors` walks from the parent up to the root, in one tree only.
  #[test]
  fn ancestors_walk_up_to_the_root() {
    let mut arena = TreeArena::new();
    let [root, middle, leaf] = nodes(&mut arena, 3)[..] else {
      unreachable!()
    };
    arena.add_child(Tree::Logical, root, middle);
    arena.add_child(Tree::Logical, middle, leaf);

    assert_eq!(arena.ancestors(Tree::Logical, leaf).collect::<Vec<_>>(), [middle, root]);
    assert_eq!(arena.ancestors(Tree::Visual, leaf).count(), 0);
  }

  // Adding a node under itself or under one of its descendants is rejected, and the tree is left as
  // it was.
  #[test]
  fn cycle_insert_is_rejected() {
    let mut arena = TreeArena::new();
    let [root, middle, leaf] = nodes(&mut arena, 3)[..] else {
      unreachable!()
    };
    arena.add_child(Tree::Logical, root, middle);
    arena.add_child(Tree::Logical, middle, leaf);

    let under_itself = catch_unwind(AssertUnwindSafe(|| arena.add_child(Tree::Logical, middle, middle)));
    let under_descendant = catch_unwind(AssertUnwindSafe(|| arena.insert_child(Tree::Logical, leaf, 0, root)));

    assert!(under_itself.is_err() && under_descendant.is_err());
    assert_eq!(arena.parent(Tree::Logical, middle), Some(root));
    assert_eq!(arena.parent(Tree::Logical, root), None);
    assert_eq!(children(&arena, Tree::Logical, leaf), []);
  }

  // The cycle check is per tree: a logical ancestor can be a visual child.
  #[test]
  fn cycle_check_looks_at_one_tree_only() {
    let mut arena = TreeArena::new();
    let [root, leaf] = nodes(&mut arena, 2)[..] else {
      unreachable!()
    };
    arena.add_child(Tree::Logical, root, leaf);

    arena.add_child(Tree::Visual, leaf, root);

    assert_eq!(arena.parent(Tree::Visual, root), Some(leaf));
  }

  // The cycle panic names both nodes and the tree.
  #[test]
  #[should_panic(expected = "would create a cycle in the Visual tree")]
  fn cycle_panic_names_the_tree() {
    let mut arena = TreeArena::new();
    let [node] = nodes(&mut arena, 1)[..] else { unreachable!() };

    arena.add_child(Tree::Visual, node, node);
  }

  // `descendants` yields a subtree in pre-order, and each tree follows its own links.
  #[test]
  fn descendants_preorder() {
    let mut arena = TreeArena::new();
    let [a, b, c, d, e] = nodes(&mut arena, 5)[..] else {
      unreachable!()
    };
    // Logical: a(b(c, d), e)
    arena.add_child(Tree::Logical, a, b);
    arena.add_child(Tree::Logical, b, c);
    arena.add_child(Tree::Logical, b, d);
    arena.add_child(Tree::Logical, a, e);
    // Visual: a(e(d, b(c)))
    arena.add_child(Tree::Visual, a, e);
    arena.add_child(Tree::Visual, e, d);
    arena.add_child(Tree::Visual, e, b);
    arena.add_child(Tree::Visual, b, c);

    assert_eq!(arena.descendants(Tree::Logical, a).collect::<Vec<_>>(), [b, c, d, e]);
    assert_eq!(arena.descendants(Tree::Visual, a).collect::<Vec<_>>(), [e, d, b, c]);
    assert_eq!(arena.descendants(Tree::Logical, c).count(), 0);
  }

  // Freeing a control frees its logical content and the visual-only parts of its template, and
  // nothing else.
  #[test]
  fn free_subtree_frees_all_descendants() {
    let mut arena = TreeArena::new();
    let [panel, button, border, presenter, text, sibling] = nodes(&mut arena, 6)[..] else {
      unreachable!()
    };
    arena.add_child(Tree::Logical, panel, button);
    arena.add_child(Tree::Logical, panel, sibling);
    arena.add_child(Tree::Logical, button, text);
    arena.add_child(Tree::Visual, panel, button);
    arena.add_child(Tree::Visual, button, border);
    arena.add_child(Tree::Visual, border, presenter);
    arena.add_child(Tree::Visual, presenter, text);

    arena.free_subtree(button);

    assert_eq!(arena.len(), 2);
    assert!(arena.contains(panel) && arena.contains(sibling));
    assert_eq!(children(&arena, Tree::Logical, panel), [sibling]);
    assert_eq!(children(&arena, Tree::Visual, panel), []);
  }

  // A node inside the subtree in one tree but under an outside parent in the other is freed, and the
  // outside parent stops listing it.
  #[test]
  fn free_subtree_detaches_nodes_from_outside_parents() {
    let mut arena = TreeArena::new();
    let [owner, popup_root, content] = nodes(&mut arena, 3)[..] else {
      unreachable!()
    };
    arena.add_child(Tree::Logical, owner, content);
    arena.add_child(Tree::Visual, popup_root, content);

    arena.free_subtree(owner);

    assert!(!arena.contains(content));
    assert_eq!(children(&arena, Tree::Visual, popup_root), []);
    assert_eq!(arena.len(), 1);
  }

  // Freeing a root frees the whole tree.
  #[test]
  fn free_subtree_of_a_root_empties_the_arena() {
    let mut arena = TreeArena::new();
    let [root, a, b] = nodes(&mut arena, 3)[..] else {
      unreachable!()
    };
    arena.add_child(Tree::Logical, root, a);
    arena.add_child(Tree::Visual, a, b);

    arena.free_subtree(root);

    assert!(arena.is_empty());
  }
}
