use artic_macros::XamlEnum;

#[derive(XamlEnum)]
enum Brush {
  Solid(u32),
  None,
}

fn main() {}
