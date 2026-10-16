use artic_core::property::Property;
use artic_macros::XamlType;

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/test")]
struct Label {
  #[xaml(content)]
  text: Property<String>,
  #[xaml(content)]
  hint: Property<String>,
}

fn main() {}
