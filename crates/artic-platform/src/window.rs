//! What a window should look like, as plain data a [`Windows`](crate::Windows) host applies.

use artic_element::SizeToContent;

/// A pair of flags, one per axis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Axes {
  /// The horizontal axis.
  pub width: bool,
  /// The vertical axis.
  pub height: bool,
}

impl Axes {
  /// Neither axis.
  pub const NONE: Axes = Axes {
    width: false,
    height: false,
  };
  /// Both axes.
  pub const BOTH: Axes = Axes {
    width: true,
    height: true,
  };
}

impl From<SizeToContent> for Axes {
  fn from(content: SizeToContent) -> Axes {
    Axes {
      width: content.width(),
      height: content.height(),
    }
  }
}
