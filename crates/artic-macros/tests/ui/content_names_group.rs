use artic_core::property::Property;
use artic_macros::{XamlProperties, XamlType};

#[derive(Debug, XamlProperties)]
struct Common {
  #[xaml]
  margin: Property<i32>,
}

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/test", content = common)]
struct Widget {
  #[xaml(flatten)]
  common: Common,
}

fn main() {}
