use crate::reactive::runtime::{Node, RUNTIME, Runtime};
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub struct Tracker {
  running: RefCell<Vec<Node>>,
  has_writes: Cell<bool>,
}

impl Tracker {
  pub const fn new() -> Self {
    Self {
      running: RefCell::new(Vec::new()),
      has_writes: Cell::new(false),
    }
  }

  fn push(&self, node: Node) {
    self.running.borrow_mut().push(node);
  }

  fn pop(&self) {
    self.running.borrow_mut().pop();
  }

  fn current(&self) -> Option<Node> {
    self.running.borrow().last().copied()
  }

  pub fn record_write(&self) {
    self.has_writes.set(true);
  }

  pub fn take_writes(&self) -> bool {
    self.has_writes.replace(false)
  }
}

impl Runtime {
  pub fn run_tracked(node: Node, action: impl FnOnce()) {
    RUNTIME.with(|runtime| {
      runtime.set_dirty(node, false);
      runtime.tracker.push(node);
    });
    let action_result = catch_unwind(AssertUnwindSafe(action));
    RUNTIME.with(|runtime| runtime.tracker.pop());
    if let Err(payload) = action_result {
      RUNTIME.with(|runtime| runtime.graph.clear_sources(node));
      resume_unwind(payload);
    }
  }

  pub fn record_read(source: Node) {
    RUNTIME.with(|runtime| {
      if let Some(reader) = runtime.tracker.current() {
        runtime.graph.add_edge(source, reader);
      }
    })
  }
}
