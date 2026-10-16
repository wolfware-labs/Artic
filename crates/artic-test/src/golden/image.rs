use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;

/// Pixels in straight RGBA, eight bits a channel, row by row from the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
  width: u32,
  height: u32,
  pixels: Vec<[u8; 4]>,
}

impl Image {
  /// An image `width` pixels wide and `height` tall, of `pixels` row by row from the top.
  ///
  /// # Panics
  ///
  /// When `pixels` doesn't hold `width × height` pixels.
  #[must_use]
  pub fn new(width: u32, height: u32, pixels: Vec<[u8; 4]>) -> Image {
    let area = width as usize * height as usize;
    assert_eq!(pixels.len(), area, "a {width}×{height} image holds {area} pixels");
    Image { width, height, pixels }
  }

  /// The width in pixels.
  #[must_use]
  pub fn width(&self) -> u32 {
    self.width
  }

  /// The height in pixels.
  #[must_use]
  pub fn height(&self) -> u32 {
    self.height
  }

  /// Every pixel, row by row from the top.
  #[must_use]
  pub fn pixels(&self) -> &[[u8; 4]] {
    &self.pixels
  }

  /// The pixel `x` from the left and `y` from the top.
  ///
  /// # Panics
  ///
  /// When `x` or `y` is outside the image.
  #[must_use]
  pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
    assert!(
      x < self.width && y < self.height,
      "({x}, {y}) is outside a {}×{} image",
      self.width,
      self.height
    );
    self.pixels[y as usize * self.width as usize + x as usize]
  }

  /// Reads an 8-bit RGBA PNG.
  ///
  /// # Errors
  ///
  /// Returns the error reading the file, or one of kind [`io::ErrorKind::InvalidData`] when it
  /// isn't an 8-bit RGBA PNG.
  pub fn load(path: &Path) -> io::Result<Image> {
    let mut reader = png::Decoder::new(BufReader::new(File::open(path)?))
      .read_info()
      .map_err(invalid)?;
    let size = reader
      .output_buffer_size()
      .ok_or_else(|| invalid_data("the image is too large".to_owned()))?;
    let mut bytes = vec![0; size];
    let info = reader.next_frame(&mut bytes).map_err(invalid)?;
    if (info.color_type, info.bit_depth) != (png::ColorType::Rgba, png::BitDepth::Eight) {
      return Err(invalid_data(format!(
        "{:?}-bit {:?} instead of 8-bit RGBA",
        info.bit_depth, info.color_type
      )));
    }
    bytes.truncate(info.buffer_size());
    Ok(Image::new(info.width, info.height, bytes.as_chunks::<4>().0.to_vec()))
  }

  /// Writes the image as an 8-bit RGBA PNG.
  ///
  /// # Errors
  ///
  /// Returns the error creating or writing the file.
  pub fn save(&self, path: &Path) -> io::Result<()> {
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), self.width, self.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(self.pixels.as_flattened())?;
    writer.finish()?;
    Ok(())
  }
}

fn invalid(error: png::DecodingError) -> io::Error {
  match error {
    png::DecodingError::IoError(error) => error,
    error => invalid_data(error.to_string()),
  }
}

fn invalid_data(message: String) -> io::Error {
  io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::golden::scratch;
  use pretty_assertions::assert_eq;

  fn checkers() -> Image {
    let pixels = (0..6)
      .map(|index| if index % 2 == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 128] })
      .collect();
    Image::new(3, 2, pixels)
  }

  // An image saved as PNG loads back the same, size and pixels.
  #[test]
  fn png_round_trips() {
    let path = scratch("png_round_trips").join("checkers.png");

    checkers().save(&path).expect("saves");

    assert_eq!(Image::load(&path).expect("loads"), checkers());
  }

  // Pixels are stored row by row from the top.
  #[test]
  fn pixel_reads_rows_from_the_top() {
    let image = Image::new(2, 2, vec![[1, 0, 0, 0], [2, 0, 0, 0], [3, 0, 0, 0], [4, 0, 0, 0]]);

    assert_eq!(image.pixel(1, 0), [2, 0, 0, 0]);
    assert_eq!(image.pixel(0, 1), [3, 0, 0, 0]);
  }

  // An image whose pixels don't fill it is a bug in the caller.
  #[test]
  #[should_panic(expected = "a 2×2 image holds 4 pixels")]
  fn new_panics_on_a_wrong_length() {
    let _ = Image::new(2, 2, vec![[0; 4]; 3]);
  }

  // Reading outside the image is a bug in the caller, as slice indexing is.
  #[test]
  #[should_panic(expected = "outside a 3×2 image")]
  fn pixel_panics_outside_the_image() {
    let _ = checkers().pixel(3, 0);
  }

  // A file that isn't a PNG reads as invalid data.
  #[test]
  fn garbage_is_invalid_data() {
    let path = scratch("garbage_is_invalid_data").join("garbage.png");
    std::fs::write(&path, b"not a png").expect("writes");

    let error = Image::load(&path).expect_err("not a png");

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
  }

  // A PNG without alpha, as an image editor may save a golden, is invalid data rather than
  // misread pixels.
  #[test]
  fn rgb_png_is_invalid_data() {
    let path = scratch("rgb_png_is_invalid_data").join("rgb.png");
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(&path).expect("creates")), 1, 1);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("header");
    writer.write_image_data(&[1, 2, 3]).expect("data");
    writer.finish().expect("finishes");

    let error = Image::load(&path).expect_err("rgb");

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
  }
}
