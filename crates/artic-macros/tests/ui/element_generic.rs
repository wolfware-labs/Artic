use artic_macros::element;

#[element]
#[derive(Debug)]
#[xaml(namespace = "https://artic.rs/test")]
struct Wrapper<T> {
  value: T,
}

fn main() {}
