//! Test fixture: registers the `Alpha` type in `XAML_TYPES` and nothing else.

use artic_core::Object;
use artic_core::property::AnyProperty;
use artic_core::tree::{NodeId, TreeArena};
use artic_registry::{TypeInfo, Whitespace};

#[derive(Debug)]
struct Alpha;

impl Object for Alpha {
  fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
}

fn new_node(arena: &mut TreeArena) -> NodeId {
  arena.alloc(Box::new(Alpha))
}

static ALPHA: TypeInfo = TypeInfo {
  name: "Alpha",
  namespace: "https://fixtures.artic.test/a",
  rust_path: "artic_registry_fixture_a::Alpha",
  new: new_node,
  content: None,
  is_collection: false,
  properties: &[],
  events: &[],
  attached_properties: &[],
  markup_extension: None,
  whitespace: Whitespace::Collapse,
};

artic_registry::register_type!(ALPHA);
