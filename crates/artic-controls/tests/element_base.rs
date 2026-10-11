// ElementBase: its properties, registered through a control that flattens it, and its layout
// helpers.

use artic_controls::{Classes, ElementBase};
use artic_core::property::Affects;
use artic_macros::XamlType;
use artic_registry::Registry;
use pretty_assertions::assert_eq;

/// A control with nothing but the base.
#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/test")]
struct Probe {
  #[xaml(flatten)]
  base: ElementBase,
}

const NAMES: [&str; 20] = [
  "Width",
  "Height",
  "MinWidth",
  "MinHeight",
  "MaxWidth",
  "MaxHeight",
  "Margin",
  "HorizontalAlignment",
  "VerticalAlignment",
  "Visibility",
  "IsEnabled",
  "Opacity",
  "RenderTransform",
  "ZIndex",
  "Classes",
  "Name",
  "Cursor",
  "Focusable",
  "IsHitTestVisible",
  "ToolTip",
];

// Every ElementBase property is found on a control that flattens it, under the `base` field.
#[test]
fn element_base_properties_registered() {
  let registry = Registry::from_link_time().expect("no duplicates");
  let info = registry
    .type_by_name("https://artic.rs/test", "Probe")
    .expect("Probe is registered");

  for name in NAMES {
    let found = info.property(name).unwrap_or_else(|| panic!("{name} is a property of Probe"));
    assert_eq!(found.group, Some("base"), "{name}");
  }
  assert_eq!(info.property("Margin").unwrap().info.affects, Affects::LAYOUT);
  assert!(info.property("IsEnabled").unwrap().info.inherits);
  assert_eq!(info.groups[0].properties.len(), NAMES.len());
}

// Defaults: no explicit size, unbounded maximum, enabled, opaque, hit-testable, no classes.
#[test]
fn element_base_defaults() {
  let base = ElementBase::default();

  assert_eq!((base.width.get(), base.max_width.get()), (None, f32::INFINITY));
  assert!(base.is_enabled.get() && base.is_hit_test_visible.get());
  assert_eq!(base.opacity.get(), 1.0);
  assert_eq!(base.classes.get(), Classes::default());
}

// Classes parse from space-separated names.
#[test]
fn classes_parse_from_names() {
  let classes: Classes = "primary  large".parse().unwrap();

  assert!(classes.contains("large") && !classes.contains("small"));
}
