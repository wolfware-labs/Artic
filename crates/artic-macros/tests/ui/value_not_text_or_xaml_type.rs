use artic_core::property::Property;
use artic_macros::XamlType;

#[derive(Debug, Default, PartialEq)]
struct Telemetry;

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui")]
struct Gauge {
  #[xaml]
  reading: Property<Telemetry>,
}

fn main() {}
