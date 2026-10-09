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
pub trait XamlCollection {
  /// Adds `child` after the children already in the collection.
  fn push_child(&mut self, child: NodeId);
}

impl XamlCollection for Vec<NodeId> {
  fn push_child(&mut self, child: NodeId) {
    self.push(child);
  }
}
