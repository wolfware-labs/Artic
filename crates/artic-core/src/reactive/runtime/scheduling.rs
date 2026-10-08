use crate::reactive::runtime::{RUNTIME, Runtime};
use std::cell::Cell;
use std::thread;

const MAX_FLUSH_PASSES: usize = 100;

pub struct Scheduler {
  is_flushing: Cell<bool>,
  batch_depth: Cell<usize>,
}

impl Scheduler {
  pub const fn new() -> Self {
    Self {
      is_flushing: Cell::new(false),
      batch_depth: Cell::new(0),
    }
  }

  fn start_flush(&self) -> bool {
    self.batch_depth.get() == 0 && !self.is_flushing.replace(true)
  }

  fn end_flush(&self) {
    self.is_flushing.set(false);
  }

  fn enter_batch(&self) {
    self.batch_depth.set(self.batch_depth.get() + 1);
  }

  fn exit_batch(&self) {
    self.batch_depth.set(self.batch_depth.get() - 1);
  }
}

impl Runtime {
  pub fn batch<TResult>(action: impl FnOnce() -> TResult) -> TResult {
    let result = {
      let _batch = BatchGuard::enter();
      action()
    };
    Self::flush_effects();
    result
  }

  pub fn flush_effects() {
    if !RUNTIME.with(|runtime| runtime.scheduler.start_flush()) {
      return;
    }
    let _flushing = FlushingGuard;
    for _ in 0..MAX_FLUSH_PASSES {
      if !RUNTIME.with(|runtime| runtime.tracker.take_writes()) {
        return;
      }
      let mut index = 0;
      while let Some(effect_id) = RUNTIME.with(|runtime| runtime.effects.nth_created(index)) {
        Self::refresh_effect(effect_id);
        index += 1;
      }
    }
    panic!("effects did not settle after {MAX_FLUSH_PASSES} passes; two effects probably write each other's inputs");
  }
}

struct FlushingGuard;

impl Drop for FlushingGuard {
  fn drop(&mut self) {
    RUNTIME.with(|runtime| {
      runtime.scheduler.end_flush();
      if thread::panicking() {
        runtime.tracker.note_write();
      }
    });
  }
}

struct BatchGuard;

impl BatchGuard {
  fn enter() -> Self {
    RUNTIME.with(|runtime| runtime.scheduler.enter_batch());
    BatchGuard
  }
}

impl Drop for BatchGuard {
  fn drop(&mut self) {
    RUNTIME.with(|runtime| runtime.scheduler.exit_batch());
  }
}
