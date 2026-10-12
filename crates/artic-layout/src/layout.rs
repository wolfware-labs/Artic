#[cfg(test)]
mod tests;

use crate::geometry::{Rect, Size};
use artic_core::property::Affects;
use artic_core::tree::{NodeId, TreeKind, UiTree};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Something layout can size and place: an element, seen by the layout pass.
///
/// Both methods take `&self`: an element's inputs are its properties, readable through a shared
/// reference, and what layout works out (sizes, rectangles, caches) is kept by the
/// [`LayoutManager`], not by the element.
pub trait Layoutable {
  /// Returns the size the element wants within `available`, its margin included.
  ///
  /// An axis of `available` may be infinite: the parent offers as much as the element wants. The
  /// returned size must be finite. Children are measured through [`LayoutCx::measure`]; a child it
  /// doesn't measure is forgotten, with its subtree, until a later measure measures it.
  fn measure(&self, cx: &mut LayoutCx<'_>, available: Size) -> Size;

  /// Places the element in `rect`, in its parent's coordinates, and its children through
  /// [`LayoutCx::arrange`], in the element's own coordinates: `(0, 0)` is its top-left corner.
  fn arrange(&self, cx: &mut LayoutCx<'_>, rect: Rect);
}

/// Finds the [`Layoutable`] of a node, for layout to measure and arrange it.
///
/// The frame pipeline answers it from the elements; tests answer it directly.
pub trait LayoutNodes {
  /// The node as layout sees it, or `None` for a node with nothing to lay out, which takes no
  /// space.
  fn layoutable<'a>(&self, tree: &'a UiTree, node: NodeId) -> Option<&'a dyn Layoutable>;
}

/// What layout knows about one node.
#[derive(Debug, Clone, Copy, Default)]
struct Slot {
  /// The constraint of the last measure.
  available: Option<Size>,
  /// What the last measure returned.
  desired: Size,
  /// The rectangle the parent last offered, in its coordinates.
  offered: Option<Rect>,
  /// The node's bounds in its parent's coordinates: the offered rectangle, unless the node set its
  /// own with [`LayoutCx::set_bounds`].
  rect: Option<Rect>,
  /// Whether the node must be arranged again even when offered the same rectangle.
  arrange_dirty: bool,
}

/// Measures and arranges a visual tree, keeping each node's results to redo only what changed.
///
/// A node is measured again when it's marked [`Affects::LAYOUT`] (a layout property changed, or its
/// visual children did) or when its parent offers it a different size. When a measured node's
/// size doesn't change, its ancestors keep theirs and only the node is arranged again.
#[derive(Debug, Default)]
pub struct LayoutManager {
  slots: HashMap<NodeId, Slot>,
}

/// What a [`LayoutManager::run`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutStats {
  /// The nodes measured, rather than answered from their cache.
  pub measured: u32,
  /// The nodes arranged, rather than skipped as unchanged.
  pub arranged: u32,
  /// The nodes whose arranged size differs from the run before, in the order they were arranged.
  /// A node arranged for the first time isn't listed, and neither is one that only moved.
  pub resized: Vec<NodeId>,
}

impl LayoutManager {
  /// Lays out `root`'s visual subtree in `available`, at `scale` device pixels per logical pixel.
  ///
  /// With `snap`, every arranged rectangle has its edges on the device-pixel grid. The root's
  /// rectangle is `available` where it's finite, and its desired size where it isn't. Results of
  /// nodes no longer in the subtree are dropped. Returns what it measured, arranged and resized.
  pub fn run(
    &mut self,
    tree: &mut UiTree,
    nodes: &impl LayoutNodes,
    root: NodeId,
    available: Size,
    scale: f32,
    snap: bool,
  ) -> LayoutStats {
    let mut depths = HashMap::new();
    let mut dirty = HashSet::new();
    let mut pending = vec![(root, 0_usize)];
    while let Some((node, depth)) = pending.pop() {
      depths.insert(node, depth);
      if tree.dirty(node).contains(Affects::LAYOUT) {
        dirty.insert(node);
      }
      pending.extend(tree.children(TreeKind::Visual, node).map(|child| (child, depth + 1)));
    }
    for node in &dirty {
      tree.clear_dirty(*node, Affects::LAYOUT);
    }
    self.slots.retain(|node, _| depths.contains_key(node));

    let mut nested = HashSet::new();
    let mut between = Vec::new();
    for node in &dirty {
      let mut path = Vec::new();
      for ancestor in tree
        .ancestors(TreeKind::Visual, *node)
        .take_while(|ancestor| depths.contains_key(ancestor))
      {
        if dirty.contains(&ancestor) {
          nested.insert(*node);
          between.append(&mut path);
          break;
        }
        path.push(ancestor);
      }
    }
    let mut by_depth: BTreeMap<usize, Vec<NodeId>> = BTreeMap::new();
    for node in dirty.iter().filter(|node| !nested.contains(*node)) {
      by_depth.entry(depths[node]).or_default().push(*node);
    }
    dirty.extend(between);
    let mut cx = LayoutCx {
      tree,
      nodes,
      slots: &mut self.slots,
      dirty,
      scale,
      snap,
      current: None,
      measured: Vec::new(),
      arranging: None,
      stats: LayoutStats::default(),
    };

    // Upward: a dirty node measures again in the size it had. If its desired size is unchanged,
    // the change stops there and only the node is arranged again; otherwise its parent is next.
    while let Some((_, deepest)) = by_depth.pop_last() {
      for node in deepest {
        if node == root || !cx.dirty.contains(&node) {
          continue;
        }
        let slot = cx.slots.get(&node).copied().unwrap_or_default();
        let changed = match slot.available {
          Some(available) => cx.measure(node, available) != slot.desired,
          None => true,
        };
        if changed
          && let Some(parent) = cx.tree.parent(TreeKind::Visual, node)
          && cx.dirty.insert(parent)
        {
          by_depth.entry(depths[&parent]).or_default().push(parent);
        }
      }
    }

    // Downward: measure and arrange from the root; clean nodes answer from their cache.
    let desired = cx.measure(root, available);
    let width = if available.width.is_finite() {
      available.width
    } else {
      desired.width
    };
    let height = if available.height.is_finite() {
      available.height
    } else {
      desired.height
    };
    cx.arrange(root, Rect::new(0.0, 0.0, width, height));

    // The nodes the upward pass stopped at, still waiting in their unchanged rectangles.
    let mut waiting: Vec<(usize, NodeId)> = cx
      .slots
      .iter()
      .filter(|(_, slot)| slot.arrange_dirty && slot.offered.is_some())
      .map(|(node, _)| (depths[node], *node))
      .collect();
    waiting.sort_by_key(|(depth, _)| *depth);
    for (_, node) in waiting {
      let slot = cx.slots[&node];
      if slot.arrange_dirty
        && let Some(rect) = slot.offered
      {
        cx.arrange(node, rect);
      }
    }
    cx.stats
  }

  /// Marks `node` to be measured again in the next [`run`](LayoutManager::run).
  pub fn invalidate_measure(tree: &mut UiTree, node: NodeId) {
    tree.mark_dirty(node, Affects::LAYOUT);
  }

  /// Marks `node` to be arranged again in the next [`run`](LayoutManager::run), in the same
  /// rectangle, for a change that moves its children without changing its size.
  pub fn invalidate_arrange(&mut self, node: NodeId) {
    self.slots.entry(node).or_default().arrange_dirty = true;
  }

  /// The size `node` last asked for, if it was measured.
  #[must_use]
  pub fn desired(&self, node: NodeId) -> Option<Size> {
    self
      .slots
      .get(&node)
      .filter(|slot| slot.available.is_some())
      .map(|slot| slot.desired)
  }

  /// The bounds `node` was last arranged in, in its parent's coordinates.
  #[must_use]
  pub fn rect(&self, node: NodeId) -> Option<Rect> {
    self.slots.get(&node).and_then(|slot| slot.rect)
  }
}

/// What an element measures and arranges its children through, during a
/// [`LayoutManager::run`].
pub struct LayoutCx<'a> {
  tree: &'a UiTree,
  nodes: &'a dyn LayoutNodes,
  slots: &'a mut HashMap<NodeId, Slot>,
  /// The nodes to measure again, whatever their parent offers.
  dirty: HashSet<NodeId>,
  scale: f32,
  snap: bool,
  /// The node being measured or arranged.
  current: Option<NodeId>,
  /// The children measured so far by each node being measured, innermost last.
  measured: Vec<HashSet<NodeId>>,
  /// The node being arranged, which only [`set_bounds`](LayoutCx::set_bounds) needs.
  arranging: Option<NodeId>,
  /// What the run has done so far.
  stats: LayoutStats,
}

impl<'a> LayoutCx<'a> {
  /// The node being measured or arranged.
  ///
  /// # Panics
  ///
  /// Panics outside a node's `measure` or `arrange`.
  #[must_use]
  pub fn node(&self) -> NodeId {
    self.current.expect("called from a node's measure or arrange")
  }

  /// The tree being laid out.
  #[must_use]
  pub fn tree(&self) -> &'a UiTree {
    self.tree
  }

  /// The visual children of the node being laid out, in order.
  pub fn children(&self) -> impl Iterator<Item = NodeId> + use<'a> {
    let (tree, node) = (self.tree, self.node());
    tree.children(TreeKind::Visual, node)
  }

  /// Device pixels per logical pixel.
  #[must_use]
  pub fn scale(&self) -> f32 {
    self.scale
  }

  /// `rect` on the device-pixel grid, or unchanged when snapping is off.
  #[must_use]
  pub fn snap(&self, rect: Rect) -> Rect {
    if self.snap { rect.snap(self.scale) } else { rect }
  }

  /// Measures `child` in `available` and returns the size it wants, its margin included. A child
  /// that isn't marked and was last measured in the same `available` answers from its cache.
  ///
  /// # Panics
  ///
  /// In debug builds, panics if `child` isn't a visual child of the node being measured (its
  /// results would be forgotten right after), if `available` has a negative or NaN axis, or if the
  /// child returns an infinite, NaN or negative size; release builds treat such an axis as zero.
  pub fn measure(&mut self, child: NodeId, available: Size) -> Size {
    debug_assert!(
      self
        .current
        .is_none_or(|parent| self.tree.parent(TreeKind::Visual, child) == Some(parent)),
      "{} measured node {child:?}, which isn't a visual child of it; measure only visual children",
      self.describe_parent(),
    );
    let available = self.checked_available(child, available);
    if let Some(siblings) = self.measured.last_mut() {
      siblings.insert(child);
    }
    let slot = self.slots.entry(child).or_default();
    if !self.dirty.contains(&child) && slot.available == Some(available) {
      return slot.desired;
    }
    self.stats.measured += 1;
    let desired = match self.nodes.layoutable(self.tree, child) {
      Some(layoutable) => {
        let parent = self.current.replace(child);
        let arranging = self.arranging.take();
        self.measured.push(HashSet::new());
        let desired = layoutable.measure(self, available);
        self.arranging = arranging;
        let measured = self.measured.pop().expect("pushed above");
        self.current = parent;
        self.forget_unmeasured(child, &measured);
        desired
      }
      None => {
        self.forget_unmeasured(child, &HashSet::new());
        Size::ZERO
      }
    };
    let desired = checked_desired(child, desired, available);
    self.dirty.remove(&child);
    let slot = self.slots.entry(child).or_default();
    slot.available = Some(available);
    slot.desired = desired;
    slot.arrange_dirty = true;
    desired
  }

  /// The size `child` asked for in its last measure, or zero if it wasn't measured.
  #[must_use]
  pub fn desired(&self, child: NodeId) -> Size {
    self.slots.get(&child).map_or(Size::ZERO, |slot| slot.desired)
  }

  /// Arranges `child` in `rect`, in the coordinates of the node being laid out, snapped to the
  /// device-pixel grid. A child already offered that rectangle, with nothing changed, is skipped.
  ///
  /// # Panics
  ///
  /// In debug builds, panics if `rect` isn't finite; release builds treat such a field as zero.
  pub fn arrange(&mut self, child: NodeId, rect: Rect) {
    let rect = self.snap(self.checked_rect(child, rect));
    let slot = self.slots.entry(child).or_default();
    if slot.offered == Some(rect) && !slot.arrange_dirty {
      return;
    }
    let before = slot.rect.map(Rect::size);
    slot.offered = Some(rect);
    slot.rect = Some(rect);
    slot.arrange_dirty = false;
    self.stats.arranged += 1;
    if let Some(layoutable) = self.nodes.layoutable(self.tree, child) {
      let parent = self.current.replace(child);
      let arranging = self.arranging.replace(child);
      layoutable.arrange(self, rect);
      self.arranging = arranging;
      self.current = parent;
    }
    let after = self.slots.get(&child).and_then(|slot| slot.rect).map(Rect::size);
    if before.is_some() && before != after {
      self.stats.resized.push(child);
    }
  }

  /// Sets the bounds of the node being arranged, in its parent's coordinates, when they aren't the
  /// rectangle its parent offered: after a margin and alignment, for instance. Its children are
  /// placed relative to these bounds. They're snapped like [`arrange`](LayoutCx::arrange)'s, and
  /// returned as recorded, so the node arranges its children in the box layout reports.
  ///
  /// # Panics
  ///
  /// Panics outside a node's `arrange`.
  pub fn set_bounds(&mut self, bounds: Rect) -> Rect {
    let node = self.arranging.expect("called from a node's arrange");
    let bounds = self.snap(self.checked_rect(node, bounds));
    self.slots.entry(node).or_default().rect = Some(bounds);
    bounds
  }

  /// Forgets what layout knew about `node`'s visual children that its measure didn't measure, and
  /// about their subtrees, so neither the upward nor the waiting pass lays them out.
  fn forget_unmeasured(&mut self, node: NodeId, measured: &HashSet<NodeId>) {
    let skipped: Vec<NodeId> = self
      .tree
      .children(TreeKind::Visual, node)
      .filter(|child| !measured.contains(child))
      .collect();
    for child in skipped {
      self.slots.remove(&child);
      for descendant in self.tree.descendants(TreeKind::Visual, child) {
        self.slots.remove(&descendant);
      }
    }
  }

  fn checked_available(&self, child: NodeId, available: Size) -> Size {
    let valid = |axis: f32| axis >= 0.0;
    debug_assert!(
      valid(available.width) && valid(available.height),
      "{} offered node {child:?} an invalid available size ({available}): each axis must be zero or more, or infinite",
      self.describe_parent(),
    );
    Size::new(
      if valid(available.width) { available.width } else { 0.0 },
      if valid(available.height) { available.height } else { 0.0 },
    )
  }

  fn checked_rect(&self, child: NodeId, rect: Rect) -> Rect {
    let fields = [rect.x, rect.y, rect.width, rect.height];
    debug_assert!(
      fields.iter().all(|field| field.is_finite()),
      "{} arranged node {child:?} in a rectangle that isn't finite ({rect})",
      self.describe_parent(),
    );
    let finite = |field: f32| if field.is_finite() { field } else { 0.0 };
    Rect::new(finite(rect.x), finite(rect.y), finite(rect.width), finite(rect.height))
  }

  fn describe_parent(&self) -> String {
    match self.current {
      Some(parent) => format!("node {parent:?}"),
      None => "the layout root".to_owned(),
    }
  }
}

impl std::fmt::Debug for LayoutCx<'_> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("LayoutCx")
      .field("current", &self.current)
      .field("scale", &self.scale)
      .field("snap", &self.snap)
      .finish_non_exhaustive()
  }
}

/// `desired`, checked to be finite and not negative.
fn checked_desired(child: NodeId, desired: Size, available: Size) -> Size {
  let valid = |axis: f32| axis.is_finite() && axis >= 0.0;
  debug_assert!(
    valid(desired.width) && valid(desired.height),
    "node {child:?} returned an invalid desired size ({desired}) for available size ({available}): it must be finite and zero or more",
  );
  Size::new(
    if valid(desired.width) { desired.width } else { 0.0 },
    if valid(desired.height) { desired.height } else { 0.0 },
  )
}
