use artic_macros::XamlType;

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://artic.rs/ui")]
struct Label {
  #[xaml]
  text: String,
}

fn main() {}
