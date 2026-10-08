use crate::reactive::runtime::ObservableId;
use std::cell::RefCell;

pub struct ReadStore {
  frames: RefCell<Vec<Vec<ObservableId>>>,
}

impl ReadStore {
  pub const fn new() -> Self {
    Self {
      frames: RefCell::new(Vec::new()),
    }
  }

  pub fn push_frame(&self) {
    self.frames.borrow_mut().push(Vec::new());
  }

  pub fn pop_frame(&self) -> Vec<ObservableId> {
    self.frames.borrow_mut().pop().unwrap_or_default()
  }

  pub fn record(&self, observable: ObservableId) {
    if let Some(observables) = self.frames.borrow_mut().last_mut()
      && !observables.contains(&observable)
    {
      observables.push(observable);
    }
  }
}
