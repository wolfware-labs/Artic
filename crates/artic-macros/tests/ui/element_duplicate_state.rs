use artic_macros::element;

/// A state type.
#[derive(Debug, Default)]
struct Presses;

#[element]
#[derive(Debug)]
#[xaml(namespace = "https://artic.rs/test")]
struct Key {
  #[element(state)]
  first: Presses,
  #[element(state)]
  second: Presses,
}

fn main() {}
