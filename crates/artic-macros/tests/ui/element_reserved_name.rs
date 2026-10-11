use artic_macros::element;

#[element]
#[derive(Debug)]
#[xaml(namespace = "https://artic.rs/test")]
struct Key {
  layout: u8,
}

fn main() {}
