use artic_core::property::Property;
use artic_core::tree::NodeId;
use artic_macros::XamlType;

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui", content = childs)]
struct Border {
  #[xaml]
  child: Property<Option<NodeId>>,
}

fn main() {}
