//! What a host reports about a surface.

use artic_render::PhysicalSize;

/// A surface's facts, reported when it appears and whenever one of them changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceInfo {
  /// Its size in device pixels.
  pub size: PhysicalSize,
  /// Device pixels per logical unit.
  pub scale: f32,
  /// The display it's on.
  pub display: DisplayId,
  /// The display's refresh rate in hertz, if the platform reports it.
  pub refresh_rate: Option<f32>,
}

/// A display, as the host identifies it. Two surfaces on one display have equal ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DisplayId(u64);

impl DisplayId {
  /// The display the host numbers `id`.
  #[must_use]
  pub const fn new(id: u64) -> DisplayId {
    DisplayId(id)
  }

  /// The host's number for the display.
  #[must_use]
  pub const fn get(self) -> u64 {
    self.0
  }
}
