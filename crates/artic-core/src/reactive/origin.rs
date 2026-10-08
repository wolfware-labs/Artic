use std::fmt::{Display, Formatter, Result};
#[cfg(debug_assertions)]
use std::panic::Location;

#[derive(Debug, Clone, Copy)]
pub struct Origin {
  #[cfg(debug_assertions)]
  location: &'static Location<'static>,
}

impl Origin {
  #[track_caller]
  pub fn caller() -> Self {
    Self {
      #[cfg(debug_assertions)]
      location: Location::caller(),
    }
  }

  pub fn panic(self, message: &str) -> ! {
    panic!("{message}{self}")
  }
}

impl Display for Origin {
  #[cfg(debug_assertions)]
  fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
    write!(formatter, " (created at {})", self.location)
  }

  #[cfg(not(debug_assertions))]
  fn fmt(&self, _formatter: &mut Formatter<'_>) -> Result {
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use static_assertions::assert_eq_size;

  #[cfg(not(debug_assertions))]
  assert_eq_size!(Origin, ());
  #[cfg(debug_assertions)]
  assert_eq_size!(Origin, &'static Location<'static>);
}
