# Design: the frame pipeline

Issue: #40. Builds on: the [host seam](host.md) (#39), dirty flags (#15), the `Dispatcher` (#16), the retained scene and `Renderer` (#33), `LayoutManager` (#41), `Element` and its property groups ([element design](element.md), #42). Unblocks: #54 (accesskit), #55 (`artic` facade), #110 (style pass), #115 (`Transitions`), #165 (debug overlay).

A **frame** turns changed state into pixels on one surface: posted work runs, the style pass applies, layout measures and arranges what changed, the scene is rebuilt from the nodes that need it, and the renderer draws it. A frame with nothing to do does no work at all, and an app with nothing to do sleeps until the OS or another thread wakes it.

## 1. Who sees what

| Who | Writes | Sees |
| --- | --- | --- |
| The facade (#55) | builds the tree from the views, then `Runner::new(tree, renderer).window(desc, root)` and `artic_platform::run(host, runner)` | `Runner`, `run`, `Hooks`, `Error` |
| Features that run inside a frame (style pass, size classes, accesskit, transitions) | a closure registered on `Hooks` | `Hooks`, `UiTree`, `SurfaceInfo`, `FrameStats` |
| Tests and the debug overlay (#165) | | `FrameStats`, per frame and per surface |
| Host adapters | nothing new | `Runner` is an ordinary `AppHandler` |

## 2. Layers

All in `artic-platform`, which depends on `artic-element` to drive the tree as `dyn Element`. It still names no concrete control and no style or theme crate.

| Type | Role |
| --- | --- |
| `Pipeline` | One surface, no host: `prepare` runs layout and composition for a root and returns a `Frame`; `present` hands it to the renderer. Owns the `LayoutManager`, the `Compositor` and the `ElementTypes` cast table |
| `Frame` | The owned `Scene`, the surface's size and scale, and the stats so far. `Send` |
| `FrameStats` | `{ measured: u32, arranged: u32, recorded: u32, rendered: bool }`, `Copy + Eq + Default` |
| `Hooks` | Per-stage lists of boxed closures: `before_layout`, `after_present`, `wants_frame` |
| `Runner<R: Renderer>` | `impl AppHandler`. Owns the `UiTree`, the renderer, the `Pipeline` and the `Hooks`, and per surface its root, latest `SurfaceInfo`, a `needs_frame` mark and the last `FrameStats` |
| `run(host, runner) -> Result<(), Error>` | Checks `renderer.accepts(host.targets())`, then `host.run(runner)`; returns the error a frame stored, if any |
| `Error` | `Unsupported(TargetKind)`: the renderer can't draw on what the host offers; `Render(RenderError)`: a frame failed |

```rust
impl<R: Renderer> Runner<R> {
  pub fn new(tree: UiTree, renderer: R) -> Self;
  pub fn window(self, desc: WindowDesc, root: NodeId) -> Self;
  pub fn hooks(&mut self) -> &mut Hooks;
  pub fn stats(&self, surface: SurfaceId) -> Option<FrameStats>;
}

impl Hooks {
  pub fn before_layout(&mut self, hook: impl FnMut(&mut UiTree, NodeId, &SurfaceInfo) + 'static);
  pub fn after_present(&mut self, hook: impl FnMut(&UiTree, NodeId, &FrameStats) + 'static);
  pub fn wants_frame(&mut self, hook: impl FnMut() -> bool + 'static);
}
```

The runner owns the tree and lends it to each stage and hook in turn, so the tree, the layout manager, the compositor and the renderer are borrowed as separate fields, never as one struct, and no hook needs a `RefCell` to reach the tree.

Only stages that run inside a frame are hooks. Features that react to host events, such as the OS color scheme (#118), safe-area and keyboard insets (#147, #150) and platform defaults (#152, #156), arrive with their `HostEvent` variants and their own handlers.

## 3. From a write to a request

A property write records that work exists and calls nothing: the `HostContext` that can ask for a frame exists only during an `AppHandler` call, and running code in the middle of a write invites re-entrancy.

`artic-core` keeps one **pending** flag per `UiTree`. Each node's properties receive a `DirtyHandle` holding the node's dirty cell and that flag, and every path that marks a node dirty sets both: a property write, `UiTree::mark_dirty`, and a child added to or removed from the visual tree. `UiTree::take_pending()` returns the flag and clears it.

The runner ends every `AppHandler` call by **settling**:

1. `Dispatcher::run_pending()`, which also flushes effects.
2. If `tree.take_pending()` is true, every live surface gets `needs_frame` and a `cx.request_frame`.

Every change happens inside some call (an event, a wake-up, a task run from one), so settling catches it. The flag is tree-wide, so a change in one window also redraws the others: each one composes again and presents, but records nothing that didn't change. A flag per surface would avoid that and can replace this one behind `take_pending`.

Pending work never keeps the loop awake by itself: the queue is empty after each settle, and nothing reads the nodes' flags to decide whether to ask for a frame, only writes set the pending flag. A hidden subtree keeps its render flags (element design §7) without asking for frames.

## 4. One frame

`AppHandler::frame(cx, surface, target, time)`:

1. If the surface is unknown or lost, return.
2. Run the dispatcher, then the `before_layout` hooks with this surface's root and `SurfaceInfo`.
3. `take_pending()`; if true, mark every surface and request frames for the others. Writes from step 2, such as the style pass's, count for this frame.
4. If this surface isn't marked, return: nothing is measured, recorded or rendered, and the loop goes back to sleep.
5. Clear the mark and `pipeline.prepare(tree, root, info)`:
   - `LayoutManager::run` at `size / scale` in logical units, with pixel snapping. It measures only layout-dirty nodes and those whose desired size a change reached.
   - `Compositor::compose` through the pipeline's render `Nodes` (section 5). It records only render-dirty nodes and reuses every other fragment.
6. `pipeline.present(renderer, frame, target)` builds the renderer's `Surface` from the frame and the lent target and renders; then the `after_present` hooks run and the surface's `FrameStats` are kept.
7. If any `wants_frame` hook returns true, mark the surface and request its next frame.

A write made during layout or rendering sets the pending flag and schedules exactly one more frame.

Pacing comes from the host and the renderer: a surface asks for its next frame only from step 7, and a renderer that presents in step with the display (as `artic-vello` does) holds that request to the display's refresh, so each surface runs at its own display's rate.

### The scene boundary

v1 renders on the UI thread, but a render thread must stay possible without changing the API (D6, spec §11). What makes it possible is data that moves by value: `prepare` reads the tree, `present` reads only the `Frame`, which owns its `Scene` and is `Send`. A render thread turns `present` into sending the `Frame`. A test asserts `Frame: Send`, so the boundary can't silently break.

## 5. The render `Nodes`

The pipeline answers the compositor's `Nodes` for any `dyn Element`, through `ElementTypes`:

- `layers(tree, node)` casts the node and calls `Element::layers` with its arranged size from `LayoutManager::rect`. The element's own transform pivots on its centre in its own coordinates, so the node's layout offset is composed after it: `transform.then(translate(rect.origin))`, or the offset alone when the element has none. `LayoutManager::rect` is in the parent's coordinates, and the compositor nests children inside their parent's layers, so offsets add up down the tree.
- `record(tree, node, builder)` casts the node and calls `Element::render` with a `RenderCx` for it.

A node that isn't an element gets default layers and records nothing.

## 6. Surfaces and events

| Call | The runner |
| --- | --- |
| `started` | opens each declared window through `cx.windows()` and maps its `SurfaceId` to its root. A host without `Windows`, such as a framebuffer, binds the declared roots to surfaces in the order `surface_changed` first reports them |
| `surface_changed` | keeps the new `SurfaceInfo` and marks the surface. A size change marks the root layout-dirty; a scale change marks the root's whole visual subtree, since snapping changes everywhere |
| `surface_lost` | flags the surface lost and calls `renderer.surface_lost`. The root stays mapped: a resume reports the same surface again |
| `event(CloseRequested)` | closes the window and forgets it; the last one closing exits |
| `event(Suspended)`, `event(Resumed)` | nothing beyond settling; the surfaces come and go through `surface_lost` and `surface_changed` |
| `woken` | settles; #79 connects the dispatcher's cross-thread `post` here |

`AppHandler` calls can't return errors. `RenderError::SurfaceLost` tells the renderer the surface is gone, marks the surface and asks for a frame, so the next one rebuilds what the renderer needs. Any other render error is kept, the runner calls `cx.exit()`, and `run` returns it as `Error::Render` once the host returns, so a launcher whose `main` returns a `Result` reports it.

## 7. Stats

Each stage counts what it did: `LayoutManager::run` returns `LayoutStats { measured, arranged }`, the compositor counts the fragments it recorded, and `present` sets `rendered`. A frame that stops at step 4 has `FrameStats::default()`. They're plain integers returned from the call that did the work, always on, so tests read exactly the frame they ran and the debug overlay reads `Runner::stats`.

## 8. Testing

| Crate | Tests |
| --- | --- |
| `artic-core` | `property_write_sets_pending`, `mark_dirty_sets_pending`, `child_change_sets_pending`, `take_pending_resets` |
| `artic-layout` | `run_reports_measured_and_arranged`: a second run with nothing dirty reports zero |
| `artic-render` | `compose_reports_recorded`: only render-dirty nodes count |
| `artic-platform` | `layout_offset_applies_after_render_transform`, `frame_is_send` |
| `artic-platform`, through `Runner`, a fake `HostContext` and a counting renderer on a `Pixels` target | `clean_frame_is_skipped`, `render_only_change_skips_layout`, `layout_change_remeasures_subtree`, `resize_invalidates_root`, `style_write_lands_in_same_frame`, `wants_frame_requests_another`, `closing_last_window_exits`, `render_error_is_returned_from_run`, `mismatched_renderer_is_rejected` |

An `artic-winit` example, `frames`, opens a window with one element through `Runner` and `artic_vello::Renderer`, changes a property once, and exits with an error if any frame runs in the last 500 ms of a 1.5 s run. The Windows and macOS CI jobs run it next to `smoke`. A quiet window, not a frame count, is what it asserts, so a transient resize at startup can't fail it.

## 9. Pull requests

| PR | Contents | Done when |
| --- | --- | --- |
| A | this document; pointers in the element and host designs | approved |
| B | `DirtyHandle`, the pending flag and `take_pending` in `artic-core`; `LayoutStats`; the compositor's recorded count | the core, layout and render tests in section 8 |
| C | `Pipeline`, `Frame`, `FrameStats`, `Hooks`, `Runner`, `run`, `Error`, the render `Nodes`; the `frames` example and its CI step | the `artic-platform` tests in section 8, and `frames` passing on Windows and macOS |
