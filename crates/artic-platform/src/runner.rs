//! The frame pipeline's `AppHandler`: when each surface draws, and what a frame runs.

use crate::{AppHandler, Error, FrameStats, Hooks, Host, HostContext, HostEvent, Pipeline, SurfaceInfo, Waker};
use artic_core::dispatcher::Dispatcher;
use artic_core::property::Affects;
use artic_core::reactive::flush_effects;
use artic_core::tree::{NodeId, TreeKind, UiTree};
use artic_core::window::WindowDesc;
use artic_element::ElementTypes;
use artic_render::{RenderError, Renderer, SurfaceId, Target};
use std::cell::RefCell;
use std::fmt::{self, Debug, Formatter};
use std::rc::Rc;
use std::time::Duration;

/// Drives a tree's frames from a host: it opens the windows, asks for a frame when something
/// changed, and runs each frame's stages.
///
/// Build it with the tree and a renderer, declare each window with its root, register hooks, and
/// hand it to [`run`].
pub struct Runner<R> {
  tree: UiTree,
  renderer: R,
  types: ElementTypes,
  hooks: Hooks,
  declared: Vec<(WindowDesc, NodeId)>,
  surfaces: Vec<Window>,
  error: Rc<RefCell<Option<Error>>>,
}

/// A window the runner draws, and what it knows about it.
struct Window {
  id: SurfaceId,
  root: NodeId,
  info: Option<SurfaceInfo>,
  needs_frame: bool,
  lost: bool,
  closing: bool,
  pipeline: Pipeline,
  stats: Option<FrameStats>,
}

impl<R: Renderer> Runner<R> {
  /// A runner for `tree`, drawing with `renderer`.
  #[must_use]
  pub fn new(tree: UiTree, renderer: R) -> Self {
    Runner {
      tree,
      renderer,
      types: ElementTypes::from_link_time(),
      hooks: Hooks::default(),
      declared: Vec::new(),
      surfaces: Vec::new(),
      error: Rc::new(RefCell::new(None)),
    }
  }

  /// Declares a window described by `desc` showing `root`'s visual subtree, opened when the host
  /// starts.
  ///
  /// # Panics
  ///
  /// Panics if `root` overlaps another window's root: the same node, inside it, or containing it.
  #[must_use]
  #[track_caller]
  pub fn window(mut self, desc: WindowDesc, root: NodeId) -> Self {
    for (_, other) in &self.declared {
      let overlaps = *other == root
        || self.tree.ancestors(TreeKind::Visual, root).any(|node| node == *other)
        || self.tree.ancestors(TreeKind::Visual, *other).any(|node| node == root);
      assert!(!overlaps, "the root {root:?} overlaps the root {other:?} of another window");
    }
    self.declared.push((desc, root));
    self
  }

  /// The hooks each frame runs.
  pub fn hooks(&mut self) -> &mut Hooks {
    &mut self.hooks
  }

  /// What `surface`'s last frame did, or `None` if it hasn't drawn.
  #[must_use]
  pub fn stats(&self, surface: SurfaceId) -> Option<FrameStats> {
    self.surfaces.iter().find(|window| window.id == surface)?.stats
  }

  fn drain() {
    loop {
      Dispatcher::run_pending();
      flush_effects();
      if Dispatcher::is_idle() {
        break;
      }
    }
  }

  fn mark_all(&mut self, cx: &mut dyn HostContext, except: Option<SurfaceId>) {
    for window in self.surfaces.iter_mut().filter(|window| !window.lost && !window.closing) {
      window.needs_frame = true;
      if Some(window.id) != except {
        cx.request_frame(window.id);
      }
    }
  }

  fn settle(&mut self, cx: &mut dyn HostContext) {
    Self::drain();
    if self.tree.take_pending() {
      self.mark_all(cx, None);
    }
  }

  fn bind(&mut self, surface: SurfaceId, root: NodeId) -> usize {
    self.surfaces.push(Window {
      id: surface,
      root,
      info: None,
      needs_frame: true,
      lost: false,
      closing: false,
      pipeline: Pipeline::new(),
      stats: None,
    });
    self.surfaces.len() - 1
  }

  fn exit_without_windows(&self, cx: &mut dyn HostContext) {
    if self.surfaces.iter().all(|window| window.closing) {
      cx.exit();
    }
  }

  #[track_caller]
  fn check_roots(&self) {
    for root in self.surfaces.iter().map(|window| window.root) {
      for ancestor in self.tree.ancestors(TreeKind::Visual, root) {
        assert!(
          !self.surfaces.iter().any(|window| window.root == ancestor),
          "the root {root:?} is inside another window's root {ancestor:?}"
        );
      }
    }
  }

  fn failed(&mut self, cx: &mut dyn HostContext, index: usize, error: RenderError) {
    if matches!(error, RenderError::SurfaceLost) {
      let id = self.surfaces[index].id;
      self.renderer.surface_lost(id);
      self.surfaces[index].needs_frame = true;
      cx.request_frame(id);
      return;
    }
    let error = Error::Render(error);
    for hook in &mut self.hooks.on_error {
      hook(&error);
    }
    *self.error.borrow_mut() = Some(error);
    cx.exit();
  }
}

impl<R: Renderer> AppHandler for Runner<R> {
  fn started(&mut self, cx: &mut dyn HostContext, _waker: Waker) {
    if let Some(windows) = cx.windows() {
      let declared = std::mem::take(&mut self.declared);
      let opened: Vec<(SurfaceId, NodeId)> = declared.iter().map(|(desc, root)| (windows.open(desc), *root)).collect();
      for (surface, root) in opened {
        self.bind(surface, root);
      }
    }
    self.settle(cx);
  }

  fn woken(&mut self, cx: &mut dyn HostContext) {
    self.settle(cx);
  }

  fn surface_changed(&mut self, cx: &mut dyn HostContext, surface: SurfaceId, info: SurfaceInfo) {
    let index = match self.surfaces.iter().position(|window| window.id == surface) {
      Some(index) => index,
      None if !self.declared.is_empty() => {
        let (_, root) = self.declared.remove(0);
        self.bind(surface, root)
      }
      None => return,
    };
    let window = &mut self.surfaces[index];
    let previous = window.info.replace(info);
    window.lost = false;
    window.needs_frame = true;
    let root = window.root;
    match previous {
      Some(previous) if previous.scale != info.scale => {
        let subtree: Vec<NodeId> = std::iter::once(root)
          .chain(self.tree.descendants(TreeKind::Visual, root))
          .collect();
        for node in subtree {
          self.tree.mark_dirty(node, Affects::LAYOUT);
        }
      }
      Some(previous) if previous.size != info.size => self.tree.mark_dirty(root, Affects::LAYOUT),
      _ => {}
    }
    cx.request_frame(surface);
    self.settle(cx);
  }

  fn surface_lost(&mut self, cx: &mut dyn HostContext, surface: SurfaceId) {
    self.renderer.surface_lost(surface);
    if let Some(index) = self.surfaces.iter().position(|window| window.id == surface) {
      let window = &mut self.surfaces[index];
      if window.closing || window.info.is_none() {
        self.surfaces.remove(index);
        self.exit_without_windows(cx);
      } else {
        window.lost = true;
      }
    }
    self.settle(cx);
  }

  fn event(&mut self, cx: &mut dyn HostContext, event: HostEvent) {
    if let HostEvent::CloseRequested { surface } = event
      && let Some(window) = self.surfaces.iter_mut().find(|window| window.id == surface)
    {
      window.closing = true;
      if let Some(windows) = cx.windows() {
        windows.close(surface);
      }
      self.exit_without_windows(cx);
    }
    self.settle(cx);
  }

  fn frame(&mut self, cx: &mut dyn HostContext, surface: SurfaceId, target: Target<'_>, time: Duration) {
    let Some(index) = self
      .surfaces
      .iter()
      .position(|window| window.id == surface && !window.lost && !window.closing)
    else {
      return;
    };
    let Some(info) = self.surfaces[index].info else {
      return;
    };
    let root = self.surfaces[index].root;
    Self::drain();
    for hook in &mut self.hooks.before_layout {
      hook(&mut self.tree, root, &info, time);
    }
    Self::drain();
    if self.tree.take_pending() {
      self.mark_all(cx, Some(surface));
    }
    self.check_roots();
    let window = &mut self.surfaces[index];
    let result = if window.needs_frame {
      window.needs_frame = false;
      let frame = window.pipeline.prepare(&mut self.tree, root, surface, &info, &self.types);
      Some(window.pipeline.present(&mut self.renderer, frame, target))
    } else {
      window.pipeline.redraw(&mut self.renderer, surface, &info, target)
    };
    let Some(result) = result else {
      return;
    };
    match result {
      Ok(stats) => {
        self.surfaces[index].stats = Some(stats);
        for hook in &mut self.hooks.after_present {
          hook(&self.tree, root, &stats);
        }
      }
      Err(error) => self.failed(cx, index, error),
    }
    let wants = self.hooks.wants_frame.iter_mut().fold(false, |wants, hook| hook() | wants);
    if wants {
      self.surfaces[index].needs_frame = true;
      cx.request_frame(surface);
    }
    self.settle(cx);
  }
}

impl<R> Debug for Runner<R> {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("Runner")
      .field("surfaces", &self.surfaces.len())
      .field("declared", &self.declared.len())
      .field("hooks", &self.hooks)
      .finish_non_exhaustive()
  }
}

/// Runs `runner` on `host`, after checking that its renderer can draw on what the host offers.
///
/// # Errors
///
/// Returns [`Error::Unsupported`] before starting if the renderer can't draw on the host's
/// targets, and [`Error::Render`] if a frame failed, once a host whose `run` blocks until the loop
/// ends returns.
pub fn run<H: Host, R: Renderer + 'static>(host: H, runner: Runner<R>) -> Result<(), Error> {
  let kind = host.targets();
  if !runner.renderer.accepts(kind) {
    return Err(Error::Unsupported(kind));
  }
  let error = Rc::clone(&runner.error);
  host.run(runner);
  error.borrow_mut().take().map_or(Ok(()), Err)
}
