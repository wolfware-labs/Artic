// A crate that depends on the artic facade marks a control #[element] through
// `artic::__private`, and its type reaches the cast table.

use artic_core::tree::UiTree;
use artic_element::{ElementImpl, ElementTypes};
use artic_macros::element;

/// A control made through the facade.
#[element]
#[derive(Debug)]
#[xaml(namespace = "https://artic.rs/test")]
struct Spacer;

impl ElementImpl for Spacer {}

// Spacer, with the facade as its crate root, is found in the cast table.
#[test]
fn facade_element_is_registered() {
  let mut tree = UiTree::new();
  let node = tree.alloc(Box::new(Spacer::default()));

  assert!(ElementTypes::from_link_time().element(tree.get(node).unwrap()).is_some());
}
