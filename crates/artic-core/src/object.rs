//! The [`Object`] trait that every node type in the tree implements.

use crate::property::AnyProperty;
use std::any::Any;
use std::fmt::Debug;

/// A value that can be stored as a node in a [`TreeArena`](crate::tree::TreeArena).
///
/// The arena stores nodes as `Box<dyn Object>`, so every node type has to implement this trait. It
/// bundles the supertraits the arena relies on:
///
/// - [`Any`] lets the arena give a node back as its concrete type, through
///   [`TreeArena::get_as`](crate::tree::TreeArena::get_as) and
///   [`TreeArena::get_as_mut`](crate::tree::TreeArena::get_as_mut). Because `Any` requires `'static`, node
///   types can't borrow non-`'static` data.
/// - [`Debug`] lets the arena itself implement `Debug`.
///
/// Its one method, [`visit_properties`](Object::visit_properties), lists the node's
/// [`Property`](crate::property::Property) fields, so the tree can mark the node dirty when one of them
/// changes and make inheritable ones follow the same property on a logical ancestor. A node type without
/// properties doesn't implement it.
///
/// # Examples
///
/// A node without properties needs only the empty `impl`:
///
/// ```
/// use artic_core::Object;
/// use artic_core::tree::TreeArena;
///
/// #[derive(Debug)]
/// struct Label {
///   text: String,
/// }
///
/// impl Object for Label {}
///
/// let mut arena = TreeArena::new();
/// let id = arena.alloc(Box::new(Label { text: "hello".into() }));
///
/// assert_eq!(arena.get_as::<Label>(id).map(|label| label.text.as_str()), Some("hello"));
/// ```
///
/// A node with properties visits each one, and changes to them then mark it dirty:
///
/// ```
/// use artic_core::Object;
/// use artic_core::property::{Affects, AnyProperty, Property, PropertyKey};
/// use artic_core::tree::TreeArena;
///
/// static FONT_SIZE: PropertyKey<f32> = PropertyKey::new("FontSize", Affects::LAYOUT, true);
///
/// #[derive(Debug)]
/// struct TextBlock {
///   font_size: Property<f32>,
/// }
///
/// impl Object for TextBlock {
///   fn visit_properties(&self, visit: &mut dyn FnMut(&dyn AnyProperty)) {
///     visit(&self.font_size);
///   }
/// }
///
/// let font_size = Property::new(&FONT_SIZE, 12.0);
/// let mut arena = TreeArena::new();
/// let id = arena.alloc(Box::new(TextBlock { font_size }));
/// arena.clear_dirty(id, Affects::LAYOUT | Affects::RENDER);
///
/// font_size.set(16.0);
/// assert_eq!(arena.dirty(id), Affects::LAYOUT);
/// ```
pub trait Object: Debug + Any {
  /// Calls `visit` with each of the node's [`Property`](crate::property::Property) fields.
  ///
  /// The tree uses it to connect the properties to their node when the node is allocated, so that
  /// changes mark the node dirty, and to find a property by its key. A node type without properties
  /// keeps the default, which visits nothing.
  fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
}
