// The runner, driven as a host would: scheduling, frames, hooks and surfaces.

mod common;

use artic_core::reactive::{Signal, effect};
use artic_core::tree::{TreeKind, UiTree};
use artic_platform::{AppHandler, FrameStats, Runner, Waker};
use artic_render::SurfaceId;
use common::{Context, Counter, LeafHandles, desc, info, leaf, panel, pixels, target};
use pretty_assertions::assert_eq;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use artic_platform::{Error, Host, HostEvent, run};
use artic_render::{RenderError, TargetKind};
const FIRST: SurfaceId = SurfaceId::new(1);

/// A runner with one window holding two leaves, started, sized and drawn once.
struct Fixture {
  runner: Runner<Counter>,
  cx: Context,
  renderer: Counter,
  top: LeafHandles,
}

impl Fixture {
  fn new(configure: impl FnOnce(&mut Runner<Counter>)) -> Fixture {
    let mut tree = UiTree::new();
    let top = leaf(&mut tree, 20.0);
    let bottom = leaf(&mut tree, 20.0);
    let (root, _) = panel(&mut tree, &[top.id, bottom.id]);
    let renderer = Counter::default();
    let mut runner = Runner::new(tree, renderer.clone()).window(desc("main"), root);
    configure(&mut runner);
    let mut cx = Context::default();
    runner.started(&mut cx, Waker::new(|| {}));
    runner.surface_changed(&mut cx, FIRST, info(64, 64, 1.0));
    let mut fixture = Fixture {
      runner,
      cx,
      renderer,
      top,
    };
    fixture.frame();
    fixture.cx.clear();
    fixture
  }

  /// Runs a frame for the window and returns its stats.
  fn frame(&mut self) -> FrameStats {
    let mut buffer = pixels();
    self.runner.frame(&mut self.cx, FIRST, target(&mut buffer), Duration::ZERO);
    self.runner.stats(FIRST).expect("the window drew")
  }

  /// Settles a change made outside any call, as a host event would.
  fn settle(&mut self) {
    self.runner.woken(&mut self.cx);
  }
}

// After a frame with nothing changed, an unrequested frame measures and records nothing, presents
// the last scene, and asks for no further frame, so the loop sleeps.
#[test]
fn clean_frame_is_skipped() {
  let mut fixture = Fixture::new(|_| {});

  let first = fixture.frame();
  let second = fixture.frame();

  assert_eq!(
    [first, second],
    [FrameStats {
      rendered: true,
      ..FrameStats::default()
    }; 2]
  );
  assert!(fixture.cx.frames.is_empty());
}

// An OS redraw, a frame nobody asked for, presents the last scene again.
#[test]
fn os_redraw_presents_the_last_scene() {
  let mut fixture = Fixture::new(|_| {});
  let before = fixture.renderer.rendered.get();

  fixture.frame();

  assert_eq!(fixture.renderer.rendered.get(), before + 1);
}

// A render-only change asks for a frame, which records the one node and lays out nothing.
#[test]
fn render_only_change_skips_layout() {
  let mut fixture = Fixture::new(|_| {});

  fixture.top.color.set(0xff0000);
  fixture.settle();
  let stats = fixture.frame();

  assert_eq!(fixture.cx.frames, [FIRST]);
  assert_eq!(
    stats,
    FrameStats {
      measured: 0,
      arranged: 0,
      recorded: 1,
      rendered: true
    }
  );
}

// A layout change measures the changed node and its parent, arranges the parent's children, and
// records only the node whose size changed.
#[test]
fn layout_change_remeasures_subtree() {
  let mut fixture = Fixture::new(|_| {});

  fixture.top.height.set(30.0);
  fixture.settle();
  let stats = fixture.frame();

  assert_eq!(
    stats,
    FrameStats {
      measured: 2,
      arranged: 3,
      recorded: 1,
      rendered: true
    }
  );
}

// A resize lays out the whole window again.
#[test]
fn resize_invalidates_root() {
  let mut fixture = Fixture::new(|_| {});

  fixture.runner.surface_changed(&mut fixture.cx, FIRST, info(48, 64, 1.0));
  let stats = fixture.frame();

  assert!(fixture.cx.frames.contains(&FIRST));
  assert_eq!((stats.measured, stats.arranged), (3, 3));
}

// Nodes layout gave a new size are recorded again although no property changed.
#[test]
fn resized_node_is_recorded_again() {
  let mut fixture = Fixture::new(|_| {});

  fixture.runner.surface_changed(&mut fixture.cx, FIRST, info(48, 64, 1.0));
  let stats = fixture.frame();

  assert_eq!(stats.recorded, 3);
}

// A write a before-layout hook makes, such as the style pass's, lands in the same frame and asks
// for no second one.
#[test]
fn style_write_lands_in_same_frame() {
  let color = Rc::new(Cell::new(None));
  let hook_color = Rc::clone(&color);
  let mut fixture = Fixture::new(|runner| {
    runner.hooks().before_layout(move |_, _, _, _| {
      if let Some(property) = hook_color.take() {
        common::set_color(property, 0x00ff00);
      }
    });
  });
  color.set(Some(fixture.top.color));

  let stats = fixture.frame();

  assert_eq!(stats.recorded, 1);
  assert!(fixture.cx.frames.is_empty());
}

// An effect that a hook's write triggers runs before the frame decides what's dirty, so its own
// write lands in the same frame too.
#[test]
fn hook_write_effects_land_in_same_frame() {
  let signal = Signal::new(0_u32);
  let armed = Rc::new(Cell::new(false));
  let hook_armed = Rc::clone(&armed);
  let mut fixture = Fixture::new(|runner| {
    runner.hooks().before_layout(move |_, _, _, _| {
      if hook_armed.replace(false) {
        signal.set(1);
      }
    });
  });
  let color = fixture.top.color;
  effect(move || {
    if signal.get() > 0 {
      color.set(0x0000ff);
    }
  });
  armed.set(true);

  let stats = fixture.frame();

  assert_eq!(stats.recorded, 1);
}

// A write after the frame was presented asks for exactly one more frame.
#[test]
fn write_after_present_requests_one_frame() {
  let color = Rc::new(Cell::new(None));
  let hook_color = Rc::clone(&color);
  let mut fixture = Fixture::new(|runner| {
    runner.hooks().after_present(move |_, _, _| {
      if let Some(property) = hook_color.take() {
        common::set_color(property, 0xffffff);
      }
    });
  });
  color.set(Some(fixture.top.color));

  fixture.top.height.set(25.0);
  fixture.settle();
  fixture.cx.clear();
  fixture.frame();
  let after_write = std::mem::take(&mut fixture.cx.frames);
  fixture.frame();

  assert_eq!(after_write, [FIRST]);
  assert!(fixture.cx.frames.is_empty());
}

// A hook that wants another frame, such as a running animation, gets one after each frame.
#[test]
fn wants_frame_requests_another() {
  let mut fixture = Fixture::new(|runner| runner.hooks().wants_frame(|| true));

  fixture.frame();

  assert_eq!(fixture.cx.frames, [FIRST]);
}

// Two windows keep their own layout and drawings: a change in one doesn't make the other record
// again.
#[test]
fn windows_keep_their_own_caches() {
  let mut tree = UiTree::new();
  let left = leaf(&mut tree, 20.0);
  let right = leaf(&mut tree, 20.0);
  let (first_root, _) = panel(&mut tree, &[left.id]);
  let (second_root, _) = panel(&mut tree, &[right.id]);
  let mut runner = Runner::new(tree, Counter::default())
    .window(desc("first"), first_root)
    .window(desc("second"), second_root);
  let mut cx = Context::default();
  let second = SurfaceId::new(2);
  runner.started(&mut cx, Waker::new(|| {}));
  for surface in [FIRST, second] {
    runner.surface_changed(&mut cx, surface, info(64, 64, 1.0));
  }
  let mut buffer = pixels();
  for surface in [FIRST, second] {
    runner.frame(&mut cx, surface, target(&mut buffer), Duration::ZERO);
  }

  left.color.set(0xff0000);
  runner.woken(&mut cx);
  for surface in [FIRST, second] {
    runner.frame(&mut cx, surface, target(&mut buffer), Duration::ZERO);
  }

  assert_eq!(runner.stats(FIRST).unwrap().recorded, 1);
  assert_eq!(runner.stats(second).unwrap().recorded, 0);
}

// A child of a hidden panel resized while hidden is recorded again once the panel shows, though
// the child itself never changed.
#[test]
fn node_resized_while_hidden_is_recorded_when_shown() {
  let mut tree = UiTree::new();
  let child = leaf(&mut tree, 20.0);
  let (inner, inner_visible) = panel(&mut tree, &[child.id]);
  let (root, _) = panel(&mut tree, &[inner]);
  let mut runner = Runner::new(tree, Counter::default()).window(desc("main"), root);
  let mut cx = Context::default();
  let mut buffer = pixels();
  runner.started(&mut cx, Waker::new(|| {}));
  runner.surface_changed(&mut cx, FIRST, info(64, 64, 1.0));
  runner.frame(&mut cx, FIRST, target(&mut buffer), Duration::ZERO);
  inner_visible.set(false);
  runner.woken(&mut cx);
  runner.frame(&mut cx, FIRST, target(&mut buffer), Duration::ZERO);
  runner.surface_changed(&mut cx, FIRST, info(48, 64, 1.0));
  runner.frame(&mut cx, FIRST, target(&mut buffer), Duration::ZERO);

  inner_visible.set(true);
  runner.woken(&mut cx);
  runner.frame(&mut cx, FIRST, target(&mut buffer), Duration::ZERO);

  assert_eq!(runner.stats(FIRST).unwrap().recorded, 2);
}

// A frame before the surface's first `surface_changed` has no size to lay out in, so it draws
// nothing.
#[test]
fn frame_before_surface_info_is_ignored() {
  let mut tree = UiTree::new();
  let (root, _) = panel(&mut tree, &[]);
  let mut runner = Runner::new(tree, Counter::default()).window(desc("main"), root);
  let mut cx = Context::default();
  let mut buffer = pixels();
  runner.started(&mut cx, Waker::new(|| {}));

  runner.frame(&mut cx, FIRST, target(&mut buffer), Duration::ZERO);

  assert_eq!(runner.stats(FIRST), None);
}

// The first frame for a surface draws in full even when the host sends it unasked.
#[test]
fn first_frame_without_a_request_draws() {
  let mut tree = UiTree::new();
  let child = leaf(&mut tree, 20.0);
  let (root, _) = panel(&mut tree, &[child.id]);
  let mut runner = Runner::new(tree, Counter::default()).window(desc("main"), root);
  let mut cx = Context::default();
  let mut buffer = pixels();
  runner.started(&mut cx, Waker::new(|| {}));
  runner.surface_changed(&mut cx, FIRST, info(64, 64, 1.0));

  runner.frame(&mut cx, FIRST, target(&mut buffer), Duration::ZERO);

  assert_eq!(runner.stats(FIRST).unwrap().recorded, 2);
}

// Two windows can't share nodes: a root inside another window's root is rejected.
#[test]
#[should_panic(expected = "overlaps")]
fn overlapping_window_roots_panic() {
  let mut tree = UiTree::new();
  let child = leaf(&mut tree, 20.0);
  let (root, _) = panel(&mut tree, &[child.id]);

  let _ = Runner::new(tree, Counter::default())
    .window(desc("outer"), root)
    .window(desc("inner"), child.id);
}

// A hook that moves one window's root under another's is caught before the next frame draws.
#[test]
#[should_panic(expected = "inside another window")]
fn reparenting_a_root_under_another_window_panics() {
  let mut tree = UiTree::new();
  let (first_root, _) = panel(&mut tree, &[]);
  let (second_root, _) = panel(&mut tree, &[]);
  let mut runner = Runner::new(tree, Counter::default())
    .window(desc("first"), first_root)
    .window(desc("second"), second_root);
  runner.hooks().before_layout(move |tree, root, _, _| {
    if root == second_root && tree.parent(TreeKind::Visual, second_root).is_none() {
      tree.add_child(TreeKind::Visual, first_root, second_root);
    }
  });
  let mut cx = Context::default();
  let mut buffer = pixels();
  runner.started(&mut cx, Waker::new(|| {}));
  runner.surface_changed(&mut cx, SurfaceId::new(2), info(64, 64, 1.0));

  runner.frame(&mut cx, SurfaceId::new(2), target(&mut buffer), Duration::ZERO);
}

// Closing the last window closes it through the host and exits.
#[test]
fn closing_last_window_exits() {
  let mut fixture = Fixture::new(|_| {});

  fixture
    .runner
    .event(&mut fixture.cx, HostEvent::CloseRequested { surface: FIRST });

  assert_eq!(fixture.cx.closed, [FIRST]);
  assert!(fixture.cx.exited);
}

// A closed window's loss, which the host reports after the close, releases what the renderer kept
// for it, and the runner forgets the window.
#[test]
fn closed_window_is_released_from_the_renderer() {
  let mut fixture = Fixture::new(|_| {});

  fixture
    .runner
    .event(&mut fixture.cx, HostEvent::CloseRequested { surface: FIRST });
  fixture.runner.surface_lost(&mut fixture.cx, FIRST);

  assert_eq!(*fixture.renderer.lost.borrow(), [FIRST]);
  assert_eq!(fixture.runner.stats(FIRST), None);
}

// A lost surface makes the renderer drop what it kept and asks for a frame that rebuilds it.
#[test]
fn surface_lost_render_error_resets_the_renderer() {
  let mut fixture = Fixture::new(|_| {});
  *fixture.renderer.fail.borrow_mut() = Some(RenderError::SurfaceLost);

  fixture.frame();

  assert_eq!(*fixture.renderer.lost.borrow(), [FIRST]);
  assert_eq!(fixture.cx.frames, [FIRST]);
}

// A host without windows binds the declared roots to surfaces in the order it reports them.
#[test]
fn windowless_host_binds_roots_in_report_order() {
  let mut tree = UiTree::new();
  let first_leaf = leaf(&mut tree, 20.0);
  let second_leaf = leaf(&mut tree, 30.0);
  let (first_root, _) = panel(&mut tree, &[first_leaf.id]);
  let (second_root, _) = panel(&mut tree, &[second_leaf.id]);
  let mut runner = Runner::new(tree, Counter::default())
    .window(desc("first"), first_root)
    .window(desc("second"), second_root);
  let drawn = Rc::new(Cell::new(None));
  let hook_drawn = Rc::clone(&drawn);
  runner.hooks().after_present(move |_, root, _| hook_drawn.set(Some(root)));
  let mut cx = Context {
    windowless: true,
    ..Context::default()
  };
  let mut buffer = pixels();
  runner.started(&mut cx, Waker::new(|| {}));
  runner.surface_changed(&mut cx, SurfaceId::new(9), info(64, 64, 1.0));

  runner.frame(&mut cx, SurfaceId::new(9), target(&mut buffer), Duration::ZERO);

  assert!(cx.opened.is_empty());
  assert_eq!(drawn.get(), Some(first_root));
}

/// A host whose surface is a pixel buffer and whose loop runs one frame.
struct OneFrame;

impl Host for OneFrame {
  fn targets(&self) -> TargetKind {
    TargetKind::Pixels
  }

  fn run(self, mut app: impl AppHandler + 'static) {
    let mut cx = Context::default();
    let mut buffer = pixels();
    app.started(&mut cx, Waker::new(|| {}));
    app.surface_changed(&mut cx, FIRST, info(64, 64, 1.0));
    app.frame(&mut cx, FIRST, target(&mut buffer), Duration::ZERO);
  }
}

/// A host that offers only window handles.
struct HandlesOnly;

impl Host for HandlesOnly {
  fn targets(&self) -> TargetKind {
    TargetKind::Handles
  }

  fn run(self, _app: impl AppHandler + 'static) {
    panic!("a host with a mismatched renderer must not start");
  }
}

/// A runner over a one-window tree drawing with `renderer`.
fn one_window(renderer: Counter) -> Runner<Counter> {
  let mut tree = UiTree::new();
  let (root, _) = panel(&mut tree, &[]);
  Runner::new(tree, renderer).window(desc("main"), root)
}

// A frame that fails to render ends the loop, and `run` returns the error.
#[test]
fn render_error_is_returned_from_run() {
  let renderer = Counter::default();
  *renderer.fail.borrow_mut() = Some(RenderError::Backend("no device".into()));

  let result = run(OneFrame, one_window(renderer));

  assert!(matches!(result, Err(Error::Render(RenderError::Backend(_)))));
}

// A failed frame reaches the error hooks, the way an app on a host whose `run` returns at once
// learns of it.
#[test]
fn render_error_reaches_on_error() {
  let renderer = Counter::default();
  *renderer.fail.borrow_mut() = Some(RenderError::Backend("no device".into()));
  let seen = Rc::new(Cell::new(0));
  let hook_seen = Rc::clone(&seen);
  let mut runner = one_window(renderer);
  runner.hooks().on_error(move |_| hook_seen.set(hook_seen.get() + 1));

  let _ = run(OneFrame, runner);

  assert_eq!(seen.get(), 1);
}

// A renderer that can't draw on what the host offers is rejected before the host starts.
#[test]
fn mismatched_renderer_is_rejected() {
  let result = run(HandlesOnly, one_window(Counter::default()));

  assert!(matches!(result, Err(Error::Unsupported(TargetKind::Handles))));
}

// A window the host lost before ever reporting it, such as one that failed to open, is forgotten,
// and with no window left the app exits rather than running with none.
#[test]
fn window_that_failed_to_open_exits() {
  let mut tree = UiTree::new();
  let (root, _) = panel(&mut tree, &[]);
  let mut runner = Runner::new(tree, Counter::default()).window(desc("main"), root);
  let mut cx = Context::default();
  runner.started(&mut cx, Waker::new(|| {}));

  runner.surface_lost(&mut cx, FIRST);

  assert!(cx.exited);
}

// With a window that failed to open, closing the other one, the last open window, exits.
#[test]
fn closing_the_last_open_window_exits_past_a_failed_one() {
  let mut tree = UiTree::new();
  let (first_root, _) = panel(&mut tree, &[]);
  let (second_root, _) = panel(&mut tree, &[]);
  let mut runner = Runner::new(tree, Counter::default())
    .window(desc("first"), first_root)
    .window(desc("second"), second_root);
  let mut cx = Context::default();
  runner.started(&mut cx, Waker::new(|| {}));
  runner.surface_changed(&mut cx, FIRST, info(64, 64, 1.0));
  runner.surface_lost(&mut cx, SurfaceId::new(2));
  assert!(!cx.exited);

  runner.event(&mut cx, HostEvent::CloseRequested { surface: FIRST });

  assert!(cx.exited);
}

// A window lost while suspended keeps its place: it comes back on resume, so the app doesn't exit.
#[test]
fn suspended_window_is_kept() {
  let mut fixture = Fixture::new(|_| {});

  fixture.runner.event(&mut fixture.cx, HostEvent::Suspended);
  fixture.runner.surface_lost(&mut fixture.cx, FIRST);
  fixture.runner.event(&mut fixture.cx, HostEvent::Resumed);
  fixture.runner.surface_changed(&mut fixture.cx, FIRST, info(64, 64, 1.0));
  let stats = fixture.frame();

  assert!(!fixture.cx.exited);
  assert_eq!(fixture.cx.frames, [FIRST]);
  assert!(stats.rendered);
}

// A hook that moves another window's root under the root being drawn is caught before this frame
// draws both.
#[test]
#[should_panic(expected = "inside another window")]
fn reparenting_another_root_under_this_one_panics() {
  let mut tree = UiTree::new();
  let (first_root, _) = panel(&mut tree, &[]);
  let (second_root, _) = panel(&mut tree, &[]);
  let mut runner = Runner::new(tree, Counter::default())
    .window(desc("first"), first_root)
    .window(desc("second"), second_root);
  runner.hooks().before_layout(move |tree, root, _, _| {
    if root == first_root && tree.parent(TreeKind::Visual, second_root).is_none() {
      tree.add_child(TreeKind::Visual, first_root, second_root);
    }
  });
  let mut cx = Context::default();
  let mut buffer = pixels();
  runner.started(&mut cx, Waker::new(|| {}));
  runner.surface_changed(&mut cx, FIRST, info(64, 64, 1.0));

  runner.frame(&mut cx, FIRST, target(&mut buffer), Duration::ZERO);
}

// A window the OS destroyed outside a suspend is gone for good, so with none left the app exits.
#[test]
fn destroyed_window_exits() {
  let mut fixture = Fixture::new(|_| {});

  fixture.runner.surface_lost(&mut fixture.cx, FIRST);

  assert!(fixture.cx.exited);
  assert_eq!(fixture.runner.stats(FIRST), None);
}
