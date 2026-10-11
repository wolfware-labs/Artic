//! The properties every control shares.

use crate::{Classes, Cursor, Visibility};
use artic_core::property::Property;
use artic_layout::geometry::{Thickness, Transform};
use artic_layout::{HorizontalAlignment, VerticalAlignment};
use artic_macros::XamlProperties;

/// The properties every control has: size, margin, alignment, visibility and the rest.
///
/// A control embeds it in a field named `base`, marked `#[xaml(flatten)]`, so its properties are
/// the control's in markup: `<Border Margin="8"/>`.
#[derive(Debug, XamlProperties)]
pub struct ElementBase {
  /// An explicit width, or `None` to size to the content.
  #[xaml(affects = layout)]
  pub width: Property<Option<f32>>,
  /// An explicit height, or `None` to size to the content.
  #[xaml(affects = layout)]
  pub height: Property<Option<f32>>,
  /// The smallest width, which wins over an explicit or maximum one.
  #[xaml(affects = layout)]
  pub min_width: Property<f32>,
  /// The smallest height, which wins over an explicit or maximum one.
  #[xaml(affects = layout)]
  pub min_height: Property<f32>,
  /// The largest width.
  #[xaml(default = f32::INFINITY, affects = layout)]
  pub max_width: Property<f32>,
  /// The largest height.
  #[xaml(default = f32::INFINITY, affects = layout)]
  pub max_height: Property<f32>,
  /// Space kept free around the element.
  #[xaml(default = Thickness::ZERO, affects = layout)]
  pub margin: Property<Thickness>,
  /// Where the element sits across its slot.
  #[xaml(affects = layout)]
  pub horizontal_alignment: Property<HorizontalAlignment>,
  /// Where the element sits along its slot's height.
  #[xaml(affects = layout)]
  pub vertical_alignment: Property<VerticalAlignment>,
  /// Whether the element is shown, hidden in its space, or collapsed to no space.
  #[xaml(affects = layout | render)]
  pub visibility: Property<Visibility>,
  /// Whether the element takes input; a disabled panel disables everything inside it.
  #[xaml(default = true, affects = render, inherits)]
  pub is_enabled: Property<bool>,
  /// How opaque the element and its descendants are, from 0 to 1.
  #[xaml(default = 1.0, affects = render)]
  pub opacity: Property<f32>,
  /// A transform applied when drawing, which layout doesn't see.
  #[xaml(affects = render)]
  pub render_transform: Property<Option<Transform>>,
  /// The drawing and hit-testing order among siblings.
  #[xaml(affects = render)]
  pub z_index: Property<i32>,
  /// The style classes style selectors match.
  #[xaml]
  pub classes: Property<Classes>,
  /// The element's name, which style selectors and code can find it by.
  #[xaml]
  pub name: Property<Option<String>>,
  /// The pointer shown over the element.
  #[xaml]
  pub cursor: Property<Cursor>,
  /// Whether the element can take keyboard focus.
  #[xaml(affects = render)]
  pub focusable: Property<bool>,
  /// Whether the element can be hit by the pointer while it's drawn.
  #[xaml(default = true, affects = render)]
  pub is_hit_test_visible: Property<bool>,
  /// Text shown when the pointer rests on the element.
  #[xaml]
  pub tool_tip: Property<Option<String>>,
}
