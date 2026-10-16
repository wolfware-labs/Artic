use artic_core::property::Property;
use artic_macros::XamlType;

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/test", content = text)]
struct Label {
  #[xaml]
  text: Property<String>,
}

fn main() {}
