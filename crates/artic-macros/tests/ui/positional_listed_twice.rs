use artic_core::property::Property;
use artic_macros::XamlType;

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui", markup_extension(positional(key, key)))]
struct Localize {
  #[xaml]
  key: Property<String>,
}

fn main() {}
