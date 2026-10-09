use artic_macros::XamlType;

#[derive(Debug, Default, Clone, PartialEq)]
struct Telemetry;

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui", attached(name = "Reading", ty = Telemetry, target = any))]
struct Gauge;

fn main() {}
