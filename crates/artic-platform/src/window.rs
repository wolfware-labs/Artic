//! What a window should look like, as plain data a [`Windows`](crate::Windows) host applies.

use artic_element::{Chrome, SizeToContent};

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

/// A window's size in logical units, one axis at a time. `None` lets the host pick that axis.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LogicalSize {
  /// The width of the content area.
  pub width: Option<f32>,
  /// The height of the content area.
  pub height: Option<f32>,
}

/// A window to open or update, as the runner builds it from a [`Surface`](artic_element::Surface)
/// and its [`Desktop`](artic_element::Desktop) settings.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowDesc {
  /// The text in its title bar.
  pub title: String,
  /// Its content area's size; the window's scale turns it into pixels.
  pub size: LogicalSize,
  /// Who draws its frame.
  pub chrome: Chrome,
  /// The axes the user can resize: those its content doesn't size.
  pub resizable: Axes,
}
