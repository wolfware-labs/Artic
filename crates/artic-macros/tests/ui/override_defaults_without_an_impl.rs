use artic_core::property::Property;
use artic_macros::XamlType;

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/test", override_defaults)]
struct Gauge {
  #[xaml]
  value: Property<i32>,
}

fn main() {}
