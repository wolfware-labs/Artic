use artic_macros::XamlType;

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui", attached(name = "Row", ty = usize, target = children, inherits))]
struct Grid;

fn main() {}
