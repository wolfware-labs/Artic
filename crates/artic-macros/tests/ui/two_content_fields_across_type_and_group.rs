use artic_core::property::Property;
use artic_macros::{XamlProperties, XamlType};

#[derive(Debug, XamlProperties)]
struct Labeled {
  #[xaml(content)]
  label: Property<String>,
}

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/test")]
struct Tag {
  #[xaml(flatten)]
  labeled: Labeled,
  #[xaml(content)]
  extra: Property<String>,
}

fn main() {}
