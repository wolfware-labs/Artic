use artic_macros::{XamlType, element};

#[element]
#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/test")]
struct Key {}

fn main() {}
