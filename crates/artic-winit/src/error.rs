//! Why the host couldn't start.

use std::fmt::{Display, Formatter, Result};

/// The host couldn't start, such as when no display is available.
#[derive(Debug)]
pub struct Error(String);

impl Error {
  /// An error with `reason`, from the windowing library's own error.
  pub(crate) fn new(reason: impl Display) -> Error {
    Error(reason.to_string())
  }
}

impl Display for Error {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
    write!(formatter, "can't start the event loop: {}", self.0)
  }
}

impl std::error::Error for Error {}
