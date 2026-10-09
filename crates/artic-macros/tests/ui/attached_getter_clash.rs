use artic_core::property::Property;
use artic_macros::XamlType;

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui", attached(name = "Row", ty = usize, target = children))]
struct Grid {
  #[xaml]
  row: Property<usize>,
}

fn main() {}
