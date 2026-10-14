// Draws composed scenes into pixel buffers through the `Renderer` trait and checks the pixels. Each
// test prints why and returns early when the machine has no GPU adapter, or only WARP, Windows'
// software DX12 adapter, on which vello's compute shaders crash (GitHub's Windows runners have
// nothing else). The tests share one renderer, so one device draws at a time.

use artic_core::Object;
use artic_core::property::AnyProperty;
use artic_core::tree::{NodeId, TreeKind, UiTree};
use artic_layout::geometry::{CornerRadius, FillRule, Path, Point, Rect, RelativePoint, RoundedRect, Transform};
use artic_render::Renderer as _;
use artic_render::{
  Color, Compositor, GradientStop, Layers, LinearGradientBrush, Nodes, PhysicalSize, PixelBuffer, PixelFormat,
  RadialGradientBrush, RenderError, SceneBuilder, SurfaceId, SurfaceView, Target, TargetKind,
};
use artic_vello::Renderer;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

const SIZE: u32 = 16;
const RED: Color = Color::rgb(255, 0, 0);
const GREEN: Color = Color::rgb(0, 255, 0);
const BLUE: Color = Color::rgb(0, 0, 255);
const CLEAR: [u8; 4] = [0, 0, 0, 0];
const OPAQUE_RED: [u8; 4] = [255, 0, 0, 255];

#[derive(Debug)]
struct Node;

impl Object for Node {
  fn visit_properties(&self, _visit: &mut dyn FnMut(&dyn AnyProperty)) {}
}

type Draw = Box<dyn Fn(&mut SceneBuilder)>;

/// Each node draws with its own closure, inside optional layers.
#[derive(Default)]
struct Drawings {
  draws: HashMap<NodeId, Draw>,
  layers: HashMap<NodeId, Layers>,
}

impl Nodes for Drawings {
  fn layers(&self, _arena: &UiTree, node: NodeId) -> Layers {
    self.layers.get(&node).cloned().unwrap_or_default()
  }

  fn record(&mut self, _arena: &UiTree, node: NodeId, builder: &mut SceneBuilder) {
    if let Some(draw) = self.draws.get(&node) {
      draw(builder);
    }
  }
}

fn fill(rect: Rect, color: Color) -> Draw {
  Box::new(move |builder| builder.fill(rect, color))
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
fn render(renderer: &mut Renderer, tree: &mut UiTree, root: NodeId, nodes: &mut Drawings, format: PixelFormat) -> Vec<[u8; 4]> {
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

fn render_one(renderer: &mut Renderer, draw: impl Fn(&mut SceneBuilder) + 'static) -> Vec<[u8; 4]> {
  let mut tree = UiTree::new();
  let root = tree.alloc(Box::new(Node));
  let mut nodes = Drawings::default();
  nodes.draws.insert(root, Box::new(draw));
  render(renderer, &mut tree, root, &mut nodes, PixelFormat::Rgba8)
}

fn assert_near(actual: [u8; 4], expected: [u8; 4], what: &str) {
  let near = actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2);
  assert!(near, "{what}: got {actual:?}, expected {expected:?}");
}

// Red up to 40%, blue from 60%: pixels near either end are exactly one color.
fn red_to_blue() -> [GradientStop; 4] {
  [
    GradientStop::new(0.0, RED),
    GradientStop::new(0.4, RED),
    GradientStop::new(0.6, BLUE),
    GradientStop::new(1.0, BLUE),
  ]
}

// A filled scene reads back exactly, in RGBA and in BGRA order.
#[test]
fn renders_scene_into_pixels() {
  with_renderer(|renderer| {
    let mut tree = UiTree::new();
    let root = tree.alloc(Box::new(Node));
    let mut nodes = Drawings::default();
    nodes.draws.insert(root, fill(full(), RED));

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
    let mut nodes = Drawings::default();
    nodes.draws.insert(root, fill(full(), BLUE));
    nodes.draws.insert(child, fill(Rect::new(0.0, 0.0, 4.0, 4.0), GREEN));
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
        clip: Some(Rect::new(0.0, 0.0, 2.0, 4.0).into()),
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
    let mut nodes = Drawings::default();
    nodes.draws.insert(root, fill(full(), Color::rgb(0, 0, 0)));
    nodes.draws.insert(overlay, fill(full(), Color::rgb(255, 255, 255)));
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

// A left-to-right gradient is its first stop at the left edge and its last at the right.
#[test]
fn linear_gradient_fills_rect() {
  with_renderer(|renderer| {
    let brush = LinearGradientBrush::new(
      RelativePoint::relative(0.0, 0.0),
      RelativePoint::relative(1.0, 0.0),
      red_to_blue(),
    );

    let pixels = render_one(renderer, move |builder| builder.fill(full(), brush.clone()));

    for y in 0..SIZE {
      assert_near(pixel(&pixels, 0, y), [255, 0, 0, 255], "left edge");
      assert_near(pixel(&pixels, SIZE - 1, y), [0, 0, 255, 255], "right edge");
    }
    let [r, _, b, _] = pixel(&pixels, SIZE / 2, 0);
    assert!(r > 2 && b > 2, "the middle blends, got red {r} and blue {b}");
  });
}

// In a box twice as wide as it is tall, radii of 50% make an ellipse: its center is the first
// stop, and the ends of both radii are the last. A circle of either radius would miss one end.
// Vello samples gradients at a pixel's corner, not its center; these pixels hold either way.
#[test]
fn radial_gradient_fills_an_ellipse() {
  with_renderer(|renderer| {
    let wide = Rect::new(0.0, 0.0, 8.0, 4.0);
    let brush = RadialGradientBrush {
      stops: red_to_blue().into(),
      ..RadialGradientBrush::default()
    };

    let pixels = render_one(renderer, move |builder| builder.fill(wide, brush.clone()));

    assert_near(pixel(&pixels, 4, 2), [255, 0, 0, 255], "center");
    assert_near(pixel(&pixels, 7, 2), [0, 0, 255, 255], "end of the horizontal radius");
    assert_near(pixel(&pixels, 4, 0), [0, 0, 255, 255], "end of the vertical radius");
  });
}

// Radii cut the corners they name, clockwise from the top-left; square corners and the middle
// stay filled.
#[test]
fn rounded_rect_leaves_corners_clear() {
  with_renderer(|renderer| {
    let card = RoundedRect::new(full(), CornerRadius::new(8.0, 0.0, 8.0, 0.0));
    let last = SIZE - 1;

    let pixels = render_one(renderer, move |builder| builder.fill(card, RED));

    assert_eq!(pixel(&pixels, 0, 0), CLEAR, "top-left, rounded");
    assert_eq!(pixel(&pixels, last, last), CLEAR, "bottom-right, rounded");
    assert_eq!(pixel(&pixels, last, 0), OPAQUE_RED, "top-right, square");
    assert_eq!(pixel(&pixels, 0, last), OPAQUE_RED, "bottom-left, square");
    assert_eq!(pixel(&pixels, 8, 8), OPAQUE_RED, "middle");
  });
}

// A triangle clip keeps what's inside it and hides the rest.
#[test]
fn path_clip_masks_drawing() {
  with_renderer(|renderer| {
    let triangle = Path::builder(Point::new(0.0, 0.0))
      .line_to(Point::new(16.0, 0.0))
      .line_to(Point::new(0.0, 16.0))
      .close()
      .build();

    let pixels = render_one(renderer, move |builder| {
      builder.with_clip(triangle.clone(), |builder| builder.fill(full(), RED));
    });

    assert_eq!(pixel(&pixels, 2, 2), OPAQUE_RED, "inside the triangle");
    assert_eq!(pixel(&pixels, 13, 13), CLEAR, "outside it");
  });
}

fn star(fill_rule: FillRule) -> Path {
  let corner = |index: u16| {
    let angle = (f32::from(index) * 144.0 - 90.0).to_radians();
    Point::new(8.0 + 8.0 * angle.cos(), 8.0 + 8.0 * angle.sin())
  };
  (1..5)
    .fold(Path::builder(corner(0)), |builder, index| builder.line_to(corner(index)))
    .close()
    .fill_rule(fill_rule)
    .build()
}

// The outline of a star winds twice around its center: the even-odd rule leaves the center
// empty, the non-zero rule fills it. Both fill the points.
#[test]
fn fill_rule_decides_a_star_center() {
  with_renderer(|renderer| {
    let even_odd = render_one(renderer, |builder| builder.fill(star(FillRule::EvenOdd), RED));
    let non_zero = render_one(renderer, |builder| builder.fill(star(FillRule::NonZero), RED));

    assert_eq!(pixel(&even_odd, 7, 7), CLEAR, "even-odd center");
    assert_eq!(pixel(&non_zero, 7, 7), OPAQUE_RED, "non-zero center");
    assert_eq!(pixel(&even_odd, 7, 4), OPAQUE_RED, "even-odd top point");
    assert_eq!(pixel(&non_zero, 7, 4), OPAQUE_RED, "non-zero top point");
  });
}

// A gradient on a path spans the path's own box: red at its left edge, blue at its right, though
// the box starts halfway across the surface.
#[test]
fn gradient_on_path_uses_its_bounds() {
  with_renderer(|renderer| {
    let square = Path::builder(Point::new(8.0, 0.0))
      .line_to(Point::new(16.0, 0.0))
      .line_to(Point::new(16.0, 8.0))
      .line_to(Point::new(8.0, 8.0))
      .close()
      .build();
    let brush = LinearGradientBrush::new(
      RelativePoint::relative(0.0, 0.0),
      RelativePoint::relative(1.0, 0.0),
      red_to_blue(),
    );

    let pixels = render_one(renderer, move |builder| builder.fill(square.clone(), brush.clone()));

    assert_near(pixel(&pixels, 8, 4), [255, 0, 0, 255], "the path's left edge");
    assert_near(pixel(&pixels, 15, 4), [0, 0, 255, 255], "the path's right edge");
    assert_eq!(pixel(&pixels, 4, 4), CLEAR, "outside the path");
  });
}

// A clip whose shape draws nothing hides what it wraps, and what's drawn after it is unaffected.
#[test]
fn undrawable_clip_hides_its_content() {
  with_renderer(|renderer| {
    let pixels = render_one(renderer, |builder| {
      builder.with_clip(Rect::new(0.0, 0.0, f32::NAN, 16.0), |builder| builder.fill(full(), RED));
      builder.fill(Rect::new(0.0, 0.0, 4.0, 4.0), BLUE);
    });

    assert_eq!(pixel(&pixels, 8, 8), CLEAR, "inside the broken clip");
    assert_eq!(pixel(&pixels, 1, 1), [0, 0, 255, 255], "drawn after it");
  });
}

// A fill closes an open figure with a straight line back to its start.
#[test]
fn open_figure_fills_closed() {
  with_renderer(|renderer| {
    let open = Path::builder(Point::new(0.0, 0.0))
      .line_to(Point::new(16.0, 0.0))
      .line_to(Point::new(0.0, 16.0))
      .build();

    let pixels = render_one(renderer, move |builder| builder.fill(open.clone(), RED));

    assert_eq!(pixel(&pixels, 2, 2), OPAQUE_RED, "inside the closed triangle");
    assert_eq!(pixel(&pixels, 13, 13), CLEAR, "outside it");
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
