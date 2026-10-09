//! Test fixture: registers the `Beta` type in `XAML_TYPES` and nothing else.

use artic_core::Object;
use artic_core::property::AnyProperty;
use artic_core::tree::{NodeId, TreeArena};
use artic_registry::TypeInfo;

#[derive(Debug)]
struct Beta;

impl Object for Beta {
  fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
}

fn new_node(arena: &mut TreeArena) -> NodeId {
  arena.alloc(Box::new(Beta))
}

static BETA: TypeInfo = TypeInfo {
  name: "Beta",
  namespace: "https://fixtures.artic.test/b",
  rust_path: "artic_registry_fixture_b::Beta",
  new: new_node,
  content: None,
  is_collection: false,
  properties: &[],
  events: &[],
  attached_properties: &[],
  markup_extension: None,
};

artic_registry::register_type!(BETA);
