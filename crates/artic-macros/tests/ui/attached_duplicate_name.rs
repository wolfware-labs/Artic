use artic_macros::XamlType;

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui")]
#[xaml(attached(name = "Row", ty = usize, target = children))]
#[xaml(attached(name = "Row", ty = i32, target = children))]
struct Grid;

fn main() {}
