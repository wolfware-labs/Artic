/// An sRGB color with straight (not premultiplied) alpha, 8 bits per channel.
///
/// A placeholder until color parsing and brushes arrive (#35).
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub struct Color {
  /// Red.
  pub r: u8,
  /// Green.
  pub g: u8,
  /// Blue.
  pub b: u8,
  /// Alpha: 0 is transparent, 255 opaque.
  pub a: u8,
}

impl Color {
  /// Creates the color with these channels.
  #[must_use]
  pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { r, g, b, a }
  }

  /// Creates the opaque color with these channels.
  #[must_use]
  pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::rgba(r, g, b, 255)
  }
}
