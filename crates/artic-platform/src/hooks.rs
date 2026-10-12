//! Code the frame runs at fixed points, registered by the facade and features above the platform.

use crate::{Error, FrameStats, SurfaceInfo};
use artic_core::tree::{NodeId, UiTree};
use std::fmt::{self, Debug, Formatter};
use std::time::Duration;

type BeforeLayout = Box<dyn FnMut(&mut UiTree, NodeId, &SurfaceInfo, Duration)>;
type AfterPresent = Box<dyn FnMut(&UiTree, NodeId, &FrameStats)>;
type WantsFrame = Box<dyn FnMut() -> bool>;
type OnError = Box<dyn FnMut(&Error)>;

/// The closures a frame runs at fixed points, each list in the order they were added.
///
/// Features the platform can't depend on, such as the style pass, size classes, accessibility
/// updates and animations, plug into the frame here.
#[derive(Default)]
pub struct Hooks {
  pub(crate) before_layout: Vec<BeforeLayout>,
  pub(crate) after_present: Vec<AfterPresent>,
  pub(crate) wants_frame: Vec<WantsFrame>,
  pub(crate) on_error: Vec<OnError>,
}

impl Hooks {
  /// Runs `hook` before each frame's layout, with the surface's root, its `SurfaceInfo` and the
  /// frame's time. Its writes land in the same frame.
  pub fn before_layout(&mut self, hook: impl FnMut(&mut UiTree, NodeId, &SurfaceInfo, Duration) + 'static) {
    self.before_layout.push(Box::new(hook));
  }

  /// Runs `hook` after each frame is presented, with the surface's root and what the frame did. A
  /// write it makes asks for one more frame.
  pub fn after_present(&mut self, hook: impl FnMut(&UiTree, NodeId, &FrameStats) + 'static) {
    self.after_present.push(Box::new(hook));
  }

  /// Asks `hook` after each frame whether the surface needs another, as a running animation does.
  pub fn wants_frame(&mut self, hook: impl FnMut() -> bool + 'static) {
    self.wants_frame.push(Box::new(hook));
  }

  /// Runs `hook` when a frame fails to render. On a host whose `run` returns at once, such as a
  /// browser's, this is how the app learns of it.
  pub fn on_error(&mut self, hook: impl FnMut(&Error) + 'static) {
    self.on_error.push(Box::new(hook));
  }
}

impl Debug for Hooks {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("Hooks")
      .field("before_layout", &self.before_layout.len())
      .field("after_present", &self.after_present.len())
      .field("wants_frame", &self.wants_frame.len())
      .field("on_error", &self.on_error.len())
      .finish()
  }
}
