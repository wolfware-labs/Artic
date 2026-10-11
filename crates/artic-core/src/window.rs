//! What a window should look like, as plain data.

/// A window to open: its title and size.
///
/// Plain data, so a `Window` element can describe itself without naming a platform; a host's
/// `Windows` capability opens it. The size is in logical units, which the window's scale turns
/// into pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowDesc {
  /// The text in the window's title bar.
  pub title: String,
  /// The width of its content area.
  pub width: f32,
  /// The height of its content area.
  pub height: f32,
}
