use artic_macros::XamlEnum;

#[derive(XamlEnum)]
enum Shape {
  Circle { radius: f32 },
  Empty,
}

fn main() {}
