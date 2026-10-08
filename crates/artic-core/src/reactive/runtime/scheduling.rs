use crate::reactive::runtime::{RUNTIME, Runtime};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

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
    RUNTIME.with(|runtime| runtime.scheduler.enter_batch());
    let action_result = catch_unwind(AssertUnwindSafe(action));
    RUNTIME.with(|runtime| runtime.scheduler.exit_batch());
    match action_result {
      Ok(result) => {
        Self::flush_effects();
        result
      }
      Err(payload) => resume_unwind(payload),
    }
  }

  pub fn flush_effects() {
    if !RUNTIME.with(|runtime| runtime.scheduler.start_flush()) {
      return;
    }
    let flush_result = catch_unwind(AssertUnwindSafe(Self::run_passes));
    RUNTIME.with(|runtime| runtime.scheduler.end_flush());
    if let Err(payload) = flush_result {
      RUNTIME.with(|runtime| runtime.tracker.record_write());
      resume_unwind(payload);
    }
  }

  fn run_passes() {
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
