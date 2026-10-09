use artic_core::Object;
use artic_core::property::AnyProperty;
use artic_core::tree::TreeArena;

#[test]
fn allocate_control() {
  #[derive(Debug, PartialEq, Eq)]
  struct Control;
  impl Object for Control {
    fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
  }

  let mut arena = TreeArena::new();
  let id = arena.alloc(Box::new(Control));
  assert_eq!(arena.get_as::<Control>(id), Some(&Control));
}
