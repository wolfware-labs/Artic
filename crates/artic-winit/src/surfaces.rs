//! The host's windows, each with its artic surface id and last reported facts.

use artic_platform::{DisplayId, SurfaceInfo};
use artic_render::SurfaceId;
use std::collections::HashMap;

/// One open window.
#[derive(Debug)]
struct Entry<K, W> {
  id: SurfaceId,
  key: K,
  window: W,
  info: Option<SurfaceInfo>,
  reported_lost: bool,
}

/// The open windows, keyed by the windowing library's id `K`, in the order they opened.
///
/// Generic over the window type `W`, so tests use `()`.
#[derive(Debug)]
pub(crate) struct Surfaces<K, W> {
  entries: Vec<Entry<K, W>>,
  next: u64,
  displays: HashMap<u64, DisplayId>,
  lost: Vec<(SurfaceId, Option<W>)>,
}

impl<K, W> Default for Surfaces<K, W> {
  fn default() -> Self {
    Self {
      entries: Vec::new(),
      next: 1,
      displays: HashMap::new(),
      lost: Vec::new(),
    }
  }
}

impl<K: PartialEq + Copy, W> Surfaces<K, W> {
  /// A new surface id, not yet tied to a window.
  pub(crate) fn reserve(&mut self) -> SurfaceId {
    let id = SurfaceId::new(self.next);
    self.next += 1;
    id
  }

  /// Adds `window`, known to the windowing library as `key`, under a new surface id.
  pub(crate) fn insert(&mut self, key: K, window: W) -> SurfaceId {
    let id = self.reserve();
    self.entries.push(Entry {
      id,
      key,
      window,
      info: None,
      reported_lost: false,
    });
    id
  }

  /// Records that surface `id` never got a window, because it couldn't be created, so it's
  /// reported lost.
  pub(crate) fn lose(&mut self, id: SurfaceId) {
    self.lost.push((id, None));
  }

  /// Closes surface `id`, keeping its window until its loss is reported, so the handles a renderer
  /// holds stay valid until then. A surface already reported lost, on suspend, is only removed.
  pub(crate) fn close(&mut self, id: SurfaceId) {
    if let Some(index) = self.entries.iter().position(|entry| entry.id == id) {
      let entry = self.entries.remove(index);
      if !entry.reported_lost {
        self.lost.push((entry.id, Some(entry.window)));
      }
    }
  }

  /// Marks every surface as reported lost, on suspend.
  pub(crate) fn mark_lost(&mut self) {
    for entry in &mut self.entries {
      entry.reported_lost = true;
    }
  }

  /// Clears the marks, on resume, when every surface is reported again.
  pub(crate) fn clear_lost_marks(&mut self) {
    for entry in &mut self.entries {
      entry.reported_lost = false;
    }
  }

  /// The surfaces to report lost, oldest first, each with its window to drop once reported,
  /// emptying the list.
  pub(crate) fn take_lost(&mut self) -> Vec<(SurfaceId, Option<W>)> {
    std::mem::take(&mut self.lost)
  }

  /// The surface of the window the library calls `key`.
  pub(crate) fn id(&self, key: K) -> Option<SurfaceId> {
    self.entries.iter().find(|entry| entry.key == key).map(|entry| entry.id)
  }

  /// The window of surface `id`.
  pub(crate) fn window(&self, id: SurfaceId) -> Option<&W> {
    self.entries.iter().find(|entry| entry.id == id).map(|entry| &entry.window)
  }

  /// The surfaces and their windows, in the order they opened.
  pub(crate) fn windows(&self) -> impl Iterator<Item = (SurfaceId, &W)> {
    self.entries.iter().map(|entry| (entry.id, &entry.window))
  }

  /// Records `info` for surface `id`, and whether it differs from what was last reported.
  pub(crate) fn update(&mut self, id: SurfaceId, info: SurfaceInfo) -> bool {
    match self.entries.iter_mut().find(|entry| entry.id == id) {
      Some(entry) if entry.info != Some(info) => {
        entry.info = Some(info);
        true
      }
      _ => false,
    }
  }

  /// Forgets the facts reported for every surface, so each is reported again, such as after the
  /// surfaces are recreated on resume.
  pub(crate) fn forget_facts(&mut self) {
    for entry in &mut self.entries {
      entry.info = None;
    }
  }

  /// Removes every surface and returns it with its window, in the order they opened.
  pub(crate) fn drain(&mut self) -> Vec<(SurfaceId, W)> {
    self.entries.drain(..).map(|entry| (entry.id, entry.window)).collect()
  }

  /// The display id for the monitor the library describes by `key`, the same for the same key.
  pub(crate) fn display_id(&mut self, key: u64) -> DisplayId {
    let next = self.displays.len() as u64 + 1;
    *self.displays.entry(key).or_insert(DisplayId::new(next))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use artic_platform::{DisplayId, SurfaceInfo};
  use artic_render::PhysicalSize;
  use pretty_assertions::assert_eq;
  use std::rc::Rc;

  /// Facts for a 100 × 100 surface at scale 1 on `display`.
  fn info(display: u64) -> SurfaceInfo {
    SurfaceInfo {
      size: PhysicalSize { width: 100, height: 100 },
      scale: 1.0,
      display: DisplayId::new(display),
      refresh_rate: Some(60.0),
    }
  }

  // Each window gets its own surface id, and its window key maps back to it.
  #[test]
  fn surfaces_get_distinct_ids() {
    let mut surfaces = Surfaces::<u32, ()>::default();

    let first = surfaces.insert(10, ());
    let second = surfaces.insert(20, ());

    assert_ne!(first, second);
    assert_eq!(
      (surfaces.id(10), surfaces.id(20), surfaces.id(30)),
      (Some(first), Some(second), None)
    );
  }

  // New facts are reported once: the same facts again are no change, and a move to another display
  // with nothing else different is one.
  #[test]
  fn display_change_is_reported_once() {
    let mut surfaces = Surfaces::<u32, ()>::default();
    let id = surfaces.insert(10, ());

    assert!(surfaces.update(id, info(1)));
    assert!(!surfaces.update(id, info(1)));
    assert!(surfaces.update(id, info(2)));
    assert!(!surfaces.update(id, info(2)));
  }

  // Draining reports what's still open, in the order it opened.
  #[test]
  fn surfaces_drain_in_open_order() {
    let mut surfaces = Surfaces::<u32, ()>::default();
    let [first, second, third] = [10, 20, 30].map(|key| surfaces.insert(key, ()));

    surfaces.close(second);

    assert_eq!(
      surfaces.drain().into_iter().map(|(id, ())| id).collect::<Vec<_>>(),
      [first, third]
    );
    assert_eq!(surfaces.id(10), None);
  }

  // A monitor keeps its display id, and two monitors get two.
  #[test]
  fn monitors_get_stable_display_ids() {
    let mut surfaces = Surfaces::<u32, ()>::default();

    let first = surfaces.display_id(111);

    assert_eq!(surfaces.display_id(111), first);
    assert_ne!(surfaces.display_id(222), first);
  }

  // A window that couldn't be created still has an id, which is reported lost.
  #[test]
  fn failed_open_reports_lost() {
    let mut surfaces = Surfaces::<u32, ()>::default();

    let id = surfaces.reserve();
    surfaces.lose(id);

    assert_eq!(surfaces.take_lost().into_iter().map(|(id, _)| id).collect::<Vec<_>>(), [id]);
    assert!(surfaces.take_lost().is_empty());
  }

  // A closed window is kept until its loss is reported, so its handles stay valid until then.
  #[test]
  fn closed_window_lives_until_reported() {
    let mut surfaces = Surfaces::<u32, Rc<()>>::default();
    let window = Rc::new(());
    let id = surfaces.insert(10, Rc::clone(&window));

    surfaces.close(id);

    assert_eq!(Rc::strong_count(&window), 2);
    let lost = surfaces.take_lost();
    assert_eq!(lost.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [id]);
    drop(lost);
    assert_eq!(Rc::strong_count(&window), 1);
  }

  // While suspended, closing a surface already reported lost reports nothing more, and closing
  // one opened during suspension, never announced, reports it lost once.
  #[test]
  fn suspended_close_reports_each_loss_once() {
    let mut surfaces = Surfaces::<u32, ()>::default();
    let before = surfaces.insert(10, ());
    surfaces.mark_lost();
    let during = surfaces.insert(20, ());

    surfaces.close(before);
    surfaces.close(during);

    assert_eq!(
      surfaces.take_lost().into_iter().map(|(id, _)| id).collect::<Vec<_>>(),
      [during]
    );
    assert_eq!((surfaces.id(10), surfaces.id(20)), (None, None));
  }

  // Resuming clears the marks, so a later close is reported again.
  #[test]
  fn resumed_surfaces_report_their_loss() {
    let mut surfaces = Surfaces::<u32, ()>::default();
    let id = surfaces.insert(10, ());
    surfaces.mark_lost();

    surfaces.clear_lost_marks();
    surfaces.close(id);

    assert_eq!(surfaces.take_lost().len(), 1);
  }

  // Forgetting the facts makes the same facts count as a change again.
  #[test]
  fn forgotten_facts_are_reported_again() {
    let mut surfaces = Surfaces::<u32, ()>::default();
    let id = surfaces.insert(10, ());
    surfaces.update(id, info(1));

    surfaces.forget_facts();

    assert!(surfaces.update(id, info(1)));
  }
}
