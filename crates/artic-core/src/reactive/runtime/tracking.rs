use crate::reactive::runtime::{ObservableId, ObserverId, RUNTIME, Runtime, SignalId};
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

struct Frame {
  observer: ObserverId,
  observables: Vec<ObservableId>,
}

pub struct Tracker {
  frames: RefCell<Vec<Frame>>,
  has_writes: Cell<bool>,
}

impl Tracker {
  pub const fn new() -> Self {
    Self {
      frames: RefCell::new(Vec::new()),
      has_writes: Cell::new(false),
    }
  }

  fn push(&self, observer: ObserverId) {
    self.frames.borrow_mut().push(Frame {
      observer,
      observables: Vec::new(),
    });
  }

  fn pop(&self) -> Vec<ObservableId> {
    self
      .frames
      .borrow_mut()
      .pop()
      .map(|frame| frame.observables)
      .unwrap_or_default()
  }

  fn current_observer(&self) -> Option<ObserverId> {
    self.frames.borrow().last().map(|frame| frame.observer)
  }

  pub fn record_write(&self) {
    self.has_writes.set(true);
  }

  pub fn take_writes(&self) -> bool {
    self.has_writes.replace(false)
  }

  fn record(&self, observable: ObservableId) {
    if let Some(frame) = self.frames.borrow_mut().last_mut()
      && !frame.observables.contains(&observable)
    {
      frame.observables.push(observable);
    }
  }
}

impl Runtime {
  pub fn run_tracked(observer_id: ObserverId, action: impl FnOnce()) -> Vec<ObservableId> {
    RUNTIME.with(|runtime| {
      runtime.observers.mark_as_clean(observer_id);
      runtime.tracker.push(observer_id);
    });
    let action_result = catch_unwind(AssertUnwindSafe(action));
    let observables = RUNTIME.with(|runtime| runtime.tracker.pop());
    if let Err(payload) = action_result {
      for observable in observables {
        observable.unsubscribe(observer_id);
      }
      resume_unwind(payload);
    }
    observables
  }

  pub fn record_read(observable: ObservableId) {
    RUNTIME.with(|runtime| runtime.tracker.record(observable))
  }

  pub(super) fn subscribe_current_observer(&self, signal_id: SignalId) {
    if let Some(observer_id) = self.tracker.current_observer() {
      self.signals.add_subscriber(signal_id, observer_id);
    }
  }
}
