use artic_macros::element;

#[element]
#[derive(Debug)]
#[xaml(namespace = "https://artic.rs/test")]
struct Key {
  #[element(status)]
  presses: u32,
}

fn main() {}
