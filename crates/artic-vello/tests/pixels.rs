// Draws composed scenes into pixel buffers through the `Renderer` trait and checks the pixels. Each
// test prints why and returns early when the machine has no GPU adapter, or only WARP, Windows'
// software DX12 adapter, on which vello's compute shaders crash (GitHub's Windows runners have
// nothing else). The tests share one renderer, so one device draws at a time.

use artic_core::Object;
use artic_core::property::AnyProperty;
use artic_core::tree::{NodeId, TreeKind, UiTree};
use artic_layout::geometry::{Rect, Transform};
use artic_render::Renderer as _;
use artic_render::{
  Clip, Color, Compositor, Layers, Nodes, PhysicalSize, PixelBuffer, PixelFormat, RenderError, SceneBuilder, SurfaceId,
  SurfaceView, Target, TargetKind,
};
use artic_vello::Renderer;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

const SIZE: u32 = 8;
const RED: Color = Color::rgb(255, 0, 0);
const GREEN: Color = Color::rgb(0, 255, 0);
const BLUE: Color = Color::rgb(0, 0, 255);

#[derive(Debug)]
struct Node;

impl Object for Node {
  fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
}

/// Each node fills a rectangle with a color, inside optional layers.
#[derive(Default)]
struct Fills {
  fills: HashMap<NodeId, (Rect, Color)>,
  layers: HashMap<NodeId, Layers>,
}

impl Nodes for Fills {
  fn layers(&self, _arena: &UiTree, node: NodeId) -> Layers {
    self.layers.get(&node).copied().unwrap_or_default()
  }

  fn record(&mut self, _arena: &UiTree, node: NodeId, builder: &mut SceneBuilder) {
    if let Some((rect, color)) = self.fills.get(&node) {
      builder.fill_rect(*rect, *color);
    }
  }
}

/// Runs `test` with the shared renderer, or prints why the machine can't run GPU tests.
fn with_renderer(test: impl FnOnce(&mut Renderer)) {
  static RENDERER: OnceLock<Mutex<Option<Renderer>>> = OnceLock::new();
  let shared = RENDERER.get_or_init(|| match Renderer::new() {
    Ok(renderer) if is_warp(&renderer) => {
      eprintln!("skipped: vello crashes on WARP, the only adapter here ({renderer:?})");
      Mutex::new(None)
    }
    Ok(renderer) => {
      eprintln!("using {renderer:?}");
      Mutex::new(Some(renderer))
    }
    Err(error) => {
      eprintln!("skipped: {error}");
      Mutex::new(None)
    }
  });
  let mut renderer = shared.lock().unwrap_or_else(PoisonError::into_inner);
  match renderer.as_mut() {
    Some(renderer) => test(renderer),
    None => eprintln!("skipped: no GPU adapter"),
  }
}

/// Whether the renderer runs on WARP: a software adapter on Windows.
fn is_warp(renderer: &Renderer) -> bool {
  cfg!(windows) && renderer.adapter_info().device_type == vello::wgpu::DeviceType::Cpu
}

/// Renders `root`'s tree into an `SIZE`×`SIZE` buffer of `format`, and returns its pixels.
fn render(renderer: &mut Renderer, tree: &mut UiTree, root: NodeId, nodes: &mut Fills, format: PixelFormat) -> Vec<[u8; 4]> {
  let scene = Compositor::default().compose(tree, root, nodes);
  let mut data = vec![0; (SIZE * SIZE * 4) as usize];
  let surface = SurfaceView {
    id: SurfaceId::new(1),
    size: PhysicalSize::new(SIZE, SIZE),
    scale: 1.0,
    target: Target::Pixels(PixelBuffer {
      data: &mut data,
      stride: (SIZE * 4) as usize,
      format,
    }),
  };
  renderer.render(&scene, surface).expect("renders");
  data.as_chunks::<4>().0.to_vec()
}

fn pixel(pixels: &[[u8; 4]], x: u32, y: u32) -> [u8; 4] {
  pixels[(y * SIZE + x) as usize]
}

fn full() -> Rect {
  Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32)
}

// A filled scene reads back exactly, in RGBA and in BGRA order.
#[test]
fn renders_scene_into_pixels() {
  with_renderer(|renderer| {
    let mut tree = UiTree::new();
    let root = tree.alloc(Box::new(Node));
    let mut nodes = Fills::default();
    nodes.fills.insert(root, (full(), RED));

    let rgba = render(renderer, &mut tree, root, &mut nodes, PixelFormat::Rgba8);
    tree.mark_dirty(root, artic_core::property::Affects::RENDER);
    let bgra = render(renderer, &mut tree, root, &mut nodes, PixelFormat::Bgra8);

    assert!(rgba.iter().all(|pixel| *pixel == [255, 0, 0, 255]), "{rgba:?}");
    assert!(bgra.iter().all(|pixel| *pixel == [0, 0, 255, 255]), "{bgra:?}");
  });
}

// A child draws over its parent, moved by the parent's transform and cut by its own clip.
#[test]
fn transform_and_clip_place_the_child() {
  with_renderer(|renderer| {
    let mut tree = UiTree::new();
    let [root, panel, child] = [(); 3].map(|()| tree.alloc(Box::new(Node)));
    tree.add_child(TreeKind::Visual, root, panel);
    tree.add_child(TreeKind::Visual, panel, child);
    let mut nodes = Fills::default();
    nodes.fills.insert(root, (full(), BLUE));
    nodes.fills.insert(child, (Rect::new(0.0, 0.0, 4.0, 4.0), GREEN));
    nodes.layers.insert(
      panel,
      Layers {
        transform: Some(Transform::translate(4.0, 4.0)),
        ..Layers::default()
      },
    );
    nodes.layers.insert(
      child,
      Layers {
        clip: Some(Clip::Rect(Rect::new(0.0, 0.0, 2.0, 4.0))),
        ..Layers::default()
      },
    );

    let pixels = render(renderer, &mut tree, root, &mut nodes, PixelFormat::Rgba8);

    assert_eq!(pixel(&pixels, 1, 1), [0, 0, 255, 255], "outside the moved child");
    assert_eq!(pixel(&pixels, 5, 5), [0, 255, 0, 255], "inside the child and its clip");
    assert_eq!(pixel(&pixels, 7, 5), [0, 0, 255, 255], "cut off by the clip");
  });
}

// An opacity layer blends its content over what's below.
#[test]
fn opacity_blends_the_layer() {
  with_renderer(|renderer| {
    let mut tree = UiTree::new();
    let [root, overlay] = [(); 2].map(|()| tree.alloc(Box::new(Node)));
    tree.add_child(TreeKind::Visual, root, overlay);
    let mut nodes = Fills::default();
    nodes.fills.insert(root, (full(), Color::rgb(0, 0, 0)));
    nodes.fills.insert(overlay, (full(), Color::rgb(255, 255, 255)));
    nodes.layers.insert(
      overlay,
      Layers {
        opacity: 0.5,
        ..Layers::default()
      },
    );

    let pixels = render(renderer, &mut tree, root, &mut nodes, PixelFormat::Rgba8);

    let [r, g, b, a] = pixel(&pixels, 3, 3);
    assert!(r.abs_diff(128) <= 2 && r == g && g == b, "half-white grey, got {r} {g} {b}");
    assert_eq!(a, 255);
  });
}

// The renderer draws on windows and pixel buffers, and rejects the rest.
#[test]
fn accepts_handles_and_pixels() {
  with_renderer(|renderer| {
    assert!(renderer.accepts(TargetKind::Handles) && renderer.accepts(TargetKind::Pixels));
    assert!(!renderer.accepts(TargetKind::Custom));
    let mut other = ();
    let surface = SurfaceView {
      id: SurfaceId::new(1),
      size: PhysicalSize::new(1, 1),
      scale: 1.0,
      target: Target::Custom(&mut other),
    };
    let error = renderer.render(&Default::default(), surface).expect_err("custom");
    assert!(matches!(error, RenderError::Unsupported(TargetKind::Custom)));
  });
}
