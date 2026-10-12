// Draws one window through the frame pipeline, changes a color once after the first frame, and
// checks that the loop then goes idle: 1.5 s after the first frame it exits with code 1 if any frame
// ran in the last 500 ms, and with code 0 otherwise. The check and the exit run on the loop's
// thread, woken by a timer through the host's `Waker`. On WARP, Windows' software adapter (all
// GitHub's Windows runners have), vello crashes, so there it draws with a renderer that draws
// nothing: the check is about when frames run, not what they show.
//
// Run with `cargo run -p artic-winit --example frames`.

use artic_core::Object;
use artic_core::property::{Affects, AnyProperty, Inheritance, Property, PropertyKey};
use artic_core::tree::UiTree;
use artic_core::window::WindowDesc;
use artic_element::{AccessCx, Element, RenderCx};
use artic_layout::geometry::{Rect, Size};
use artic_layout::{LayoutCx, Layoutable};
use artic_platform::{AppHandler, Host as _, HostContext, HostEvent, Runner, SurfaceInfo, Waker};
use artic_render::{Color, RenderError, Renderer, Scene, SceneBuilder, Surface, SurfaceId, Target, TargetKind};
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

static RED: PropertyKey<u8> = PropertyKey::new("Red", Affects::RENDER, Inheritance::None);

/// Fills its whole box with a shade of red.
#[derive(Debug)]
struct Swatch {
  red: Property<u8>,
}

impl Object for Swatch {
  fn visit_properties(&self, visit: &mut dyn FnMut(&dyn AnyProperty)) {
    visit(&self.red);
  }
}

impl Layoutable for Swatch {
  fn measure(&self, _cx: &mut LayoutCx<'_>, available: Size) -> Size {
    available
  }

  fn arrange(&self, _cx: &mut LayoutCx<'_>, _rect: Rect) {}
}

impl Element for Swatch {
  fn render(&self, cx: &mut RenderCx<'_>, scene: &mut SceneBuilder) {
    let size = cx.size();
    scene.fill_rect(Rect::new(0.0, 0.0, size.width, size.height), Color::rgb(self.red.get(), 0, 0));
  }

  fn accessibility(&self, _cx: &AccessCx<'_>) -> accesskit::Node {
    accesskit::Node::new(accesskit::Role::GenericContainer)
  }
}

artic_element::element_type!(Swatch);

/// Runs `runner` and judges, 1.5 s after the first frame, whether the last 500 ms were quiet.
struct Idle<R> {
  runner: Runner<R>,
  waker: Option<Waker>,
  frames: Vec<Instant>,
  quiet: Rc<Cell<Option<bool>>>,
}

impl<R: Renderer> AppHandler for Idle<R> {
  fn started(&mut self, cx: &mut dyn HostContext, waker: Waker) {
    self.waker = Some(waker.clone());
    self.runner.started(cx, waker);
  }

  fn woken(&mut self, cx: &mut dyn HostContext) {
    self.runner.woken(cx);
    let Some(first) = self.frames.first().copied() else {
      return;
    };
    if self.quiet.get().is_none() && first.elapsed() >= Duration::from_millis(1500) {
      let busy = self
        .frames
        .iter()
        .filter(|at| at.duration_since(first) > Duration::from_millis(1000))
        .count();
      println!("{} frames, {busy} in the last 500 ms", self.frames.len());
      self.quiet.set(Some(self.frames.len() >= 2 && busy == 0));
      cx.exit();
    }
  }

  fn surface_changed(&mut self, cx: &mut dyn HostContext, surface: SurfaceId, info: SurfaceInfo) {
    self.runner.surface_changed(cx, surface, info);
  }

  fn surface_lost(&mut self, cx: &mut dyn HostContext, surface: SurfaceId) {
    self.runner.surface_lost(cx, surface);
  }

  fn event(&mut self, cx: &mut dyn HostContext, event: HostEvent) {
    self.runner.event(cx, event);
  }

  fn frame(&mut self, cx: &mut dyn HostContext, surface: SurfaceId, target: Target<'_>, time: Duration) {
    self.runner.frame(cx, surface, target, time);
    if self.frames.is_empty()
      && let Some(waker) = self.waker.clone()
    {
      std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        waker.wake();
      });
    }
    self.frames.push(Instant::now());
  }
}

/// Accepts window handles and draws nothing.
struct Blank;

impl Renderer for Blank {
  fn accepts(&self, kind: TargetKind) -> bool {
    kind == TargetKind::Handles
  }

  fn render(&mut self, _scene: &Scene, _surface: Surface<'_>) -> Result<(), RenderError> {
    Ok(())
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let renderer = artic_vello::Renderer::new().map_err(|error| format!("no GPU renderer: {error}"))?;
  if cfg!(windows) && renderer.adapter_info().device_type == vello::wgpu::DeviceType::Cpu {
    println!("WARP, on which vello crashes: drawing nothing");
    check(Blank)
  } else {
    check(renderer)
  }
}

/// Runs the app with `renderer` and returns whether it went idle.
fn check(renderer: impl Renderer + 'static) -> Result<(), Box<dyn std::error::Error>> {
  let mut tree = UiTree::new();
  let red = Property::new(&RED, 128);
  let root = tree.alloc(Box::new(Swatch { red }));
  let mut runner = Runner::new(tree, renderer).window(
    WindowDesc {
      title: "artic frames".into(),
      width: 320.0,
      height: 240.0,
    },
    root,
  );
  let changed = Rc::new(Cell::new(false));
  runner.hooks().after_present(move |_, _, stats| {
    println!("frame: {stats:?}");
    if !changed.replace(true) {
      red.set(255);
    }
  });
  runner.hooks().on_error(|error| eprintln!("frame failed: {error}"));

  let quiet = Rc::new(Cell::new(None));
  let idle = Idle {
    runner,
    waker: None,
    frames: Vec::new(),
    quiet: Rc::clone(&quiet),
  };
  artic_winit::Host::new()?.run(idle);
  match quiet.get() {
    Some(true) => Ok(()),
    Some(false) => Err("frames ran after the app should have gone idle".into()),
    None => Err("the loop ended before the check".into()),
  }
}
