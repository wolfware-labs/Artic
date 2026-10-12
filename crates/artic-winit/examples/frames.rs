// Draws one window through the frame pipeline, changes a color once after the first frame, and
// checks that the loop then goes idle: it exits with code 1 if any frame runs in the last 500 ms of
// a 1.5 s run, and with code 0 otherwise.
//
// Run with `cargo run -p artic-winit --example frames`.

use artic_core::Object;
use artic_core::property::{Affects, AnyProperty, Inheritance, Property, PropertyKey};
use artic_core::tree::UiTree;
use artic_core::window::WindowDesc;
use artic_element::{AccessCx, Element, RenderCx};
use artic_layout::geometry::{Rect, Size};
use artic_layout::{LayoutCx, Layoutable};
use artic_render::{Color, SceneBuilder};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut tree = UiTree::new();
  let red = Property::new(&RED, 128);
  let root = tree.alloc(Box::new(Swatch { red }));
  let renderer = artic_vello::Renderer::new().map_err(|error| format!("no GPU renderer: {error}"))?;
  let mut runner = artic_platform::Runner::new(tree, renderer).window(
    WindowDesc {
      title: "artic frames".into(),
      width: 320.0,
      height: 240.0,
    },
    root,
  );

  let start = Instant::now();
  let frames: Arc<Mutex<Vec<Duration>>> = Arc::default();
  let log = Arc::clone(&frames);
  let changed = Rc::new(Cell::new(false));
  runner.hooks().after_present(move |_, _, stats| {
    println!("frame: {stats:?}");
    log.lock().unwrap().push(start.elapsed());
    if !changed.replace(true) {
      red.set(255);
    }
  });

  std::thread::spawn(move || {
    std::thread::sleep(Duration::from_millis(1500));
    let frames = frames.lock().unwrap();
    let busy = frames.iter().filter(|at| **at > Duration::from_millis(1000)).count();
    println!("{} frames, {busy} in the last 500 ms", frames.len());
    let code = if frames.len() >= 2 && busy == 0 { 0 } else { 1 };
    std::process::exit(code);
  });

  artic_platform::run(artic_winit::Host::new()?, runner)?;
  Ok(())
}
