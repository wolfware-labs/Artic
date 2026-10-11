// Opens a window and draws a small scene with `artic_vello::Renderer`, through the same
// `Compositor` and `Renderer` seam artic's frame pipeline will use: a panel with a clipped header,
// a half-transparent overlay, and a square spinning at the display's refresh rate.
//
// Run with `cargo run -p artic-vello --example window`. Until the `artic-winit` host exists
// (#39), this example drives `winit` itself.

use artic_core::Object;
use artic_core::property::AnyProperty;
use artic_core::tree::{NodeId, TreeKind, UiTree};
use artic_layout::geometry::{Rect, Transform};
use artic_render::Renderer as _;
use artic_render::{Clip, Color, Compositor, Layers, Nodes, PhysicalSize, SceneBuilder, Surface, SurfaceId, Target};
use artic_vello::Renderer;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

fn main() {
  let renderer = match Renderer::new() {
    Ok(renderer) => renderer,
    Err(error) => {
      eprintln!("can't start the renderer: {error}");
      std::process::exit(1);
    }
  };
  let event_loop = EventLoop::new().expect("an event loop");
  let mut app = Demo::new(renderer);
  event_loop.run_app(&mut app).expect("the event loop runs");
}

#[derive(Debug)]
struct Node;

impl Object for Node {
  fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
}

/// The demo's nodes, laid out by hand until layout (#41) exists.
struct Scene {
  background: NodeId,
  panel: NodeId,
  header: NodeId,
  overlay: NodeId,
  spinner: NodeId,
  /// The window's size in logical units.
  width: f32,
  height: f32,
  /// The spinner's angle, in radians.
  angle: f32,
}

impl Nodes for Scene {
  fn layers(&self, _arena: &UiTree, node: NodeId) -> Layers {
    if node == self.panel {
      Layers {
        transform: Some(Transform::translate(40.0, 40.0)),
        clip: Some(Clip::Rect(Rect::new(0.0, 0.0, 320.0, 200.0))),
        ..Layers::default()
      }
    } else if node == self.overlay {
      Layers {
        transform: Some(Transform::translate(200.0, 140.0)),
        opacity: 0.6,
        ..Layers::default()
      }
    } else if node == self.spinner {
      let center = Transform::translate(self.width - 120.0, self.height - 120.0);
      Layers {
        transform: Some(Transform::rotate(self.angle).then(center)),
        ..Layers::default()
      }
    } else {
      Layers::default()
    }
  }

  fn record(&mut self, _arena: &UiTree, node: NodeId, builder: &mut SceneBuilder) {
    if node == self.background {
      builder.fill_rect(Rect::new(0.0, 0.0, self.width, self.height), Color::rgb(24, 26, 33));
    } else if node == self.panel {
      builder.fill_rect(Rect::new(0.0, 0.0, 320.0, 200.0), Color::rgb(236, 239, 244));
    } else if node == self.header {
      // Wider than the panel on purpose: the panel's clip cuts it at the panel's edge.
      builder.fill_rect(Rect::new(0.0, 0.0, 600.0, 40.0), Color::rgb(59, 130, 246));
    } else if node == self.overlay {
      builder.fill_rect(Rect::new(0.0, 0.0, 240.0, 160.0), Color::rgb(236, 72, 153));
    } else if node == self.spinner {
      builder.fill_rect(Rect::new(-50.0, -50.0, 100.0, 100.0), Color::rgb(250, 204, 21));
    }
  }
}

struct Demo {
  renderer: Renderer,
  compositor: Compositor,
  tree: UiTree,
  scene: Scene,
  window: Option<Arc<Window>>,
  start: Instant,
}

impl Demo {
  fn new(renderer: Renderer) -> Demo {
    let mut tree = UiTree::new();
    let [background, panel, header, overlay, spinner] = [(); 5].map(|()| tree.alloc(Box::new(Node)));
    tree.add_child(TreeKind::Visual, background, panel);
    tree.add_child(TreeKind::Visual, panel, header);
    tree.add_child(TreeKind::Visual, background, overlay);
    tree.add_child(TreeKind::Visual, background, spinner);
    Demo {
      renderer,
      compositor: Compositor::default(),
      tree,
      scene: Scene {
        background,
        panel,
        header,
        overlay,
        spinner,
        width: 0.0,
        height: 0.0,
        angle: 0.0,
      },
      window: None,
      start: Instant::now(),
    }
  }

  fn redraw(&mut self) {
    let Some(window) = self.window.clone() else { return };
    let physical = window.inner_size();
    let scale = window.scale_factor() as f32;
    let (width, height) = (physical.width as f32 / scale, physical.height as f32 / scale);
    if (width, height) != (self.scene.width, self.scene.height) {
      // Only the background depends on the size, so only it records again.
      self
        .tree
        .mark_dirty(self.scene.background, artic_core::property::Affects::RENDER);
      (self.scene.width, self.scene.height) = (width, height);
    }
    self.scene.angle = self.start.elapsed().as_secs_f32();

    let scene = self
      .compositor
      .compose(&mut self.tree, self.scene.background, &mut self.scene);
    let surface = Surface {
      id: SurfaceId::new(1),
      size: PhysicalSize::new(physical.width, physical.height),
      scale,
      target: Target::Handles(window.as_ref()),
    };
    if let Err(error) = self.renderer.render(&scene, surface) {
      eprintln!("frame skipped: {error}");
    }
    window.request_redraw();
  }
}

impl ApplicationHandler for Demo {
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    if self.window.is_some() {
      return;
    }
    let attributes = Window::default_attributes()
      .with_title("artic · vello")
      .with_inner_size(winit::dpi::LogicalSize::new(720.0, 480.0));
    let window = event_loop.create_window(attributes).expect("a window");
    self.window = Some(Arc::new(window));
  }

  fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
    match event {
      WindowEvent::CloseRequested => {
        self.renderer.surface_lost(SurfaceId::new(1));
        self.window = None;
        event_loop.exit();
      }
      WindowEvent::RedrawRequested => self.redraw(),
      WindowEvent::Resized(_) => {
        if let Some(window) = &self.window {
          window.request_redraw();
        }
      }
      _ => {}
    }
  }
}
