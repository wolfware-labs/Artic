use artic_core::event::Event;
use artic_macros::XamlProperties;

#[derive(Debug, XamlProperties)]
struct Common {
  #[xaml]
  clicked: Event<()>,
}

fn main() {}
