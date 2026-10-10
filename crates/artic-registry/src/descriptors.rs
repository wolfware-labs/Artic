use artic_core::property::Affects;
use artic_core::tree::{NodeId, UiTree};

/// What the registry knows about one Rust type that XAML can name.
///
/// Every field is plain data that can be built at compile time, so a descriptor is a `static` item:
/// the `XamlType` derive emits one per type, and a type can also be described by hand.
#[derive(Debug)]
pub struct TypeInfo {
  /// The name markup uses for the type, such as `Border`. Lookups are exact and case-sensitive.
  pub name: &'static str,
  /// The XML namespace the name belongs to, such as `https://artic.rs/ui`.
  pub namespace: &'static str,
  /// The full Rust path of the type, such as `artic_controls::Border`. A `using:` namespace prefix
  /// is resolved through it.
  pub rust_path: &'static str,
  /// Creates a node of this type with its default values and returns its id.
  pub new: fn(&mut UiTree) -> NodeId,
  /// The field that receives content written directly inside the element, if any.
  pub content: Option<&'static str>,
  /// Whether the type is a collection that accepts child elements.
  pub is_collection: bool,
  /// The type's settable XAML properties.
  pub properties: &'static [PropertyInfo],
  /// The type's XAML events.
  pub events: &'static [EventInfo],
  /// The attached properties this type owns, set on other elements as `Owner.Name`.
  pub attached_properties: &'static [AttachedPropertyInfo],
  /// The arguments the type takes when used as a markup extension, or `None` if it isn't one.
  pub markup_extension: Option<&'static MarkupExtensionInfo>,
  /// How the parser treats whitespace in text written inside the element.
  pub whitespace: Whitespace,
}

/// How the parser treats whitespace in text content.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum Whitespace {
  /// Runs of whitespace collapse to one space, and leading and trailing whitespace is dropped.
  #[default]
  Collapse,
  /// Text keeps its whitespace as written.
  Significant,
}

/// A Rust type that XAML can name, described by a [`TypeInfo`].
///
/// `#[derive(XamlType)]` implements it. A type that implements it can also be the value of a
/// property, as an object element.
pub trait XamlType {
  /// The type's registry descriptor.
  const TYPE_INFO: &'static TypeInfo;
}

impl TypeInfo {
  /// Whether the type can be used as a markup extension, such as `{Binding}`.
  #[must_use]
  pub const fn is_markup_extension(&self) -> bool {
    self.markup_extension.is_some()
  }
}

/// A settable XAML property of a type.
///
/// `name`, `affects` and `inherits` repeat the property's
/// [`PropertyKey`](artic_core::property::PropertyKey), copied at compile time through its `const`
/// accessors, so the key stays the single source of that metadata.
#[derive(Debug)]
pub struct PropertyInfo {
  /// The property's name in markup, such as `Padding`.
  pub name: &'static str,
  /// The Rust field that holds the property, such as `padding`.
  pub field: &'static str,
  /// The Rust path of the property's value type, such as `artic_layout::Thickness`.
  pub value_type: &'static str,
  /// The frame stages a change to the property has to redo.
  pub affects: Affects,
  /// Whether the property takes its value from a logical ancestor when it has none of its own.
  pub inherits: bool,
}

/// A XAML event of a type.
#[derive(Debug)]
pub struct EventInfo {
  /// The event's name in markup, such as `PointerPressed`.
  pub name: &'static str,
  /// The Rust path of the arguments handlers receive.
  pub args_type: &'static str,
}

/// A property owned by one type and set on elements of other types, such as `Grid.Row`.
#[derive(Debug)]
pub struct AttachedPropertyInfo {
  /// The property's name after the owner, such as `Row` in `Grid.Row`.
  pub name: &'static str,
  /// The Rust path of the property's value type.
  pub value_type: &'static str,
  /// Which elements it may be set on.
  pub target: AttachedTarget,
}

/// Which elements an attached property may be set on.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AttachedTarget {
  /// Any element, such as `Accessibility.Name`.
  Any,
  /// The logical children of an element of the owner type, such as `Grid.Row` on a grid's
  /// children.
  ChildrenOfOwner,
}

/// The arguments a markup extension accepts.
#[derive(Debug)]
pub struct MarkupExtensionInfo {
  /// Arguments given by position, in order, as in `{Binding Name}`.
  pub positional: &'static [ArgInfo],
  /// Arguments given by name, as in `{Binding Path=Name}`.
  pub named: &'static [ArgInfo],
}

/// One argument of a markup extension.
#[derive(Debug)]
pub struct ArgInfo {
  /// The argument's name.
  pub name: &'static str,
  /// The Rust path of the argument's value type.
  pub value_type: &'static str,
}

#[cfg(test)]
mod tests {
  use super::*;
  use artic_core::Object;
  use artic_core::property::AnyProperty;
  use artic_core::property::{Property, PropertyKey};
  use pretty_assertions::assert_eq;
  use static_assertions::assert_impl_all;

  assert_impl_all!(TypeInfo: Sync);

  static PADDING: PropertyKey<f32> = PropertyKey::new("Padding", Affects::LAYOUT, false);

  #[derive(Debug)]
  struct Border {
    _padding: Property<f32>,
  }

  impl Object for Border {
    fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  }

  fn new_border(tree: &mut UiTree) -> NodeId {
    tree.alloc(Box::new(Border {
      _padding: Property::new(&PADDING, 0.0),
    }))
  }

  static BORDER: TypeInfo = TypeInfo {
    name: "Border",
    namespace: "https://artic.rs/ui",
    rust_path: "artic_controls::Border",
    new: new_border,
    content: Some("child"),
    is_collection: false,
    properties: &[PropertyInfo {
      name: PADDING.name(),
      field: "padding",
      value_type: "f32",
      affects: PADDING.affects(),
      inherits: PADDING.inherits(),
    }],
    events: &[EventInfo {
      name: "PointerPressed",
      args_type: "artic_core::PointerEvent",
    }],
    attached_properties: &[],
    markup_extension: None,
    whitespace: Whitespace::Collapse,
  };

  // A descriptor written as a `static` copies its property metadata from the property's key at
  // compile time, and its constructor creates a node.
  #[test]
  fn static_descriptor_reads_metadata_from_the_key() {
    let mut tree = UiTree::new();

    let node = (BORDER.new)(&mut tree);

    assert_eq!(BORDER.properties[0].name, "Padding");
    assert_eq!(BORDER.properties[0].affects, Affects::LAYOUT);
    assert!(!BORDER.is_markup_extension());
    assert!(tree.get_as::<Border>(node).is_some());
  }
}
