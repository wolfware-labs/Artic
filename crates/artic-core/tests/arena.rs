use artic_core::{Object, TreeArena};

#[test]
fn allocate_control() {
  #[derive(Debug, PartialEq, Eq)]
  struct Control;
  impl Object for Control {}

  let mut arena = TreeArena::new();
  let id = arena.alloc(Box::new(Control));
  assert_eq!(arena.get_as::<Control>(id), Some(&Control));
}
