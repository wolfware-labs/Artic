//! winit's window events, translated into what the host reports.
//!
//! [`facts`] pulls the values out of a winit event, and [`translate`] decides from those values
//! alone, so tests build its input without a display.

use artic_render::PhysicalSize;
use winit::event::WindowEvent;
use winit::window::Window;

/// The parts of a winit window event the host uses.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WindowEventFacts {
  Resized { width: u32, height: u32 },
  ScaleFactorChanged { scale: f64, width: u32, height: u32 },
  CloseRequested,
  RedrawRequested,
  Moved,
  Destroyed,
  Other,
}

/// What a window event means to the host.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Translated {
  Changed { size: PhysicalSize, scale: Option<f32> },
  Close,
  Redraw,
  Moved,
  Destroyed,
  Ignored,
}

/// The values the host needs from `event` on `window`. A scale change carries a size writer tests
/// can't build, so its new size is read from the window.
pub(crate) fn facts(event: &WindowEvent, window: &Window) -> WindowEventFacts {
  match event {
    WindowEvent::Resized(size) => WindowEventFacts::Resized {
      width: size.width,
      height: size.height,
    },
    WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
      let size = window.inner_size();
      WindowEventFacts::ScaleFactorChanged {
        scale: *scale_factor,
        width: size.width,
        height: size.height,
      }
    }
    WindowEvent::CloseRequested => WindowEventFacts::CloseRequested,
    WindowEvent::RedrawRequested => WindowEventFacts::RedrawRequested,
    WindowEvent::Moved(_) => WindowEventFacts::Moved,
    WindowEvent::Destroyed => WindowEventFacts::Destroyed,
    _ => WindowEventFacts::Other,
  }
}

/// What `facts` means: new surface facts, a close request, a frame, a move, the window's
/// destruction, or nothing.
pub(crate) fn translate(facts: &WindowEventFacts) -> Translated {
  match *facts {
    WindowEventFacts::Resized { width, height } => Translated::Changed {
      size: PhysicalSize { width, height },
      scale: None,
    },
    WindowEventFacts::ScaleFactorChanged { scale, width, height } => Translated::Changed {
      size: PhysicalSize { width, height },
      scale: Some(scale as f32),
    },
    WindowEventFacts::CloseRequested => Translated::Close,
    WindowEventFacts::RedrawRequested => Translated::Redraw,
    WindowEventFacts::Moved => Translated::Moved,
    WindowEventFacts::Destroyed => Translated::Destroyed,
    WindowEventFacts::Other => Translated::Ignored,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use artic_render::PhysicalSize;
  use pretty_assertions::assert_eq;

  // Resizes and scale changes become new surface facts; close, redraw, moves and destruction keep
  // their meaning; anything else is ignored.
  #[test]
  fn winit_event_translation() {
    let size = |width, height| PhysicalSize { width, height };

    assert_eq!(
      translate(&WindowEventFacts::Resized { width: 800, height: 600 }),
      Translated::Changed {
        size: size(800, 600),
        scale: None
      }
    );
    assert_eq!(
      translate(&WindowEventFacts::ScaleFactorChanged {
        scale: 2.0,
        width: 1600,
        height: 1200
      }),
      Translated::Changed {
        size: size(1600, 1200),
        scale: Some(2.0)
      }
    );
    assert_eq!(translate(&WindowEventFacts::CloseRequested), Translated::Close);
    assert_eq!(translate(&WindowEventFacts::RedrawRequested), Translated::Redraw);
    assert_eq!(translate(&WindowEventFacts::Moved), Translated::Moved);
    assert_eq!(translate(&WindowEventFacts::Destroyed), Translated::Destroyed);
    assert_eq!(translate(&WindowEventFacts::Other), Translated::Ignored);
  }
}
