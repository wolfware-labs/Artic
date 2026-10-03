#[derive(Debug, Eq, PartialEq)]
pub struct Observer {
  is_dirty: bool,
}

impl Observer {
  pub fn mark_as_dirty(&mut self) {
    self.is_dirty = true;
  }
}
