use artic_core::event::Event;
use artic_core::property::Property;
use artic_macros::XamlType;

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui", markup_extension(positional(key, changed)))]
struct Localize {
  #[xaml]
  key: Property<String>,
  #[xaml]
  changed: Event<()>,
}

fn main() {}
