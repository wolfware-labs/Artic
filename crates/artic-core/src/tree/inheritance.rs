//! How inheritable properties take their value from the logical tree.
//!
//! A property whose key inherits, such as `FontSize`, follows the same property on its nearest
//! logical ancestor that has one: that ancestor's value fills its `Inherited` level. This file finds
//! that ancestor for every inheritable property of a subtree, and is run again whenever a logical
//! edit moves the subtree, so each property follows its new ancestor. Nodes without the property
//! are skipped, and inheritance never follows the visual tree.
//!
//! It holds no state of its own: it reads the objects and the logical tree's links.

use crate::property::AnyProperty;
use crate::tree::NodeId;
use crate::tree::hierarchy::Tree;
use crate::tree::objects::Objects;
use std::iter::once;

/// Makes each inheritable property of `root` and its logical descendants follow its nearest logical
/// ancestor.
pub(super) fn resolve(objects: &Objects, logical: &Tree, root: NodeId) {
  for node in once(root).chain(logical.descendants(root)) {
    objects.object(node).visit_properties(&mut |property| {
      if property.inherits() {
        inherit_from_nearest_ancestor(objects, logical, node, property);
      }
    });
  }
}

fn inherit_from_nearest_ancestor(objects: &Objects, logical: &Tree, node: NodeId, property: &dyn AnyProperty) {
  for ancestor in logical.ancestors(node) {
    let mut found = false;
    objects.object(ancestor).visit_properties(&mut |candidate| {
      if !found && candidate.key_id() == property.key_id() {
        property.inherit_from(Some(candidate));
        found = true;
      }
    });
    if found {
      return;
    }
  }
  property.inherit_from(None);
}
