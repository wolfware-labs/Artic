//! The fields that receive an element's child elements.
//!
//! A `#[xaml]` field that is neither a `Property` nor an `Event` is a collection: when markup writes
//! elements inside the type's element, each child's node is pushed into it, in order, through
//! [`XamlCollection`].

use crate::tree::NodeId;

/// A field that receives the child elements written inside a XAML element, in order.
///
/// A panel's children are the usual example: `<StackPanel><A/><B/></StackPanel>` pushes `A`, then
/// `B`, into the panel's collection field.
///
/// # Examples
///
/// ```
/// use artic_core::Object;
/// use artic_core::property::AnyProperty;
/// use artic_core::tree::{NodeId, TreeArena, XamlCollection};
///
/// #[derive(Debug)]
/// struct Item;
/// impl Object for Item {
///   fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
/// }
///
/// let mut arena = TreeArena::new();
/// let mut children: Vec<NodeId> = Vec::new();
/// children.push_child(arena.alloc(Box::new(Item)));
/// assert_eq!(children.len(), 1);
/// ```
#[diagnostic::on_unimplemented(
  message = "`{Self}` can't hold child elements",
  label = "`#[xaml]` on a field that isn't a `Property` or an `Event` makes it a collection of children",
  note = "to make this field a XAML property, declare it as `Property<{Self}>`"
)]
pub trait XamlCollection {
  /// Adds `child` after the children already in the collection.
  fn push_child(&mut self, child: NodeId);
}

impl XamlCollection for Vec<NodeId> {
  fn push_child(&mut self, child: NodeId) {
    self.push(child);
  }
}
