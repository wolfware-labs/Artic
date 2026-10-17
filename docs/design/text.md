# Design: Text shaping and fonts

Issue: #37. Builds on: [backends](backends.md) (#33), [brushes](brush.md) (#35), [shapes, strokes, clips and shadows](shapes.md) (#36), [golden images](golden-images.md) (#38), [layout tests](layout-tests.md) (#56), [frames](frame.md). Unblocks: #46, #47, #175, #171.

Text goes from a string, a font family list and a width to positioned glyphs broken into lines, with a size layout can use. A control shapes its text while it's laid out, keeps the result, and draws that same result when it records its scene, so text is shaped once per change and never by a renderer.

```rust
let paragraph = cx.text().paragraph("Hello, world", &style, TextLimits::wrap(200.0));
// later, in render:
scene.text(&paragraph, Point::ZERO, foreground);
```

The issue predates the [backends design](backends.md), which removed `Renderer::measure_text` and moved shaping out of the renderer. This document replaces the issue's entry point, its "renderer owns the contexts" note and its glyph cache; section 8 lists the acceptance criteria that change.

## 1. Crates

`parley` shapes and breaks lines, and `fontique` lists the system's fonts and maps family names to them. Both live in a new crate, `artic-text`, so no other crate compiles them. A renderer draws glyph runs from font data and never sees `parley`.

| Crate | Gains | Depends on |
| --- | --- | --- |
| `artic-layout` | the `TextEngine` trait and the value types of section 2, `NoText`, `MeasureCx::text()` and `ArrangeCx::text()`, and an engine argument to `LayoutManager::run` and `measure` | unchanged |
| `artic-text` (new) | `Engine`, which implements `TextEngine` with `parley` and `fontique` | `artic-core`, `artic-layout` |
| `artic-render` | `Command::Text` and `SceneBuilder::text` | unchanged |
| `artic-platform` | the runner owns one engine and lends it to each surface's `Pipeline::prepare` and `Pipeline::measure` | unchanged |
| `artic-vello` | draws `Command::Text` | unchanged |
| `artic-test`, `artic` | create the engine: the test font's and the system's | add `artic-text` |
| `artic-winit` | the `frames` example creates `Engine::system()` | a dev-dependency on `artic-text` |

`artic-text` sits in layer 2, beside `artic-render`. The data it produces, `Paragraph`, is defined in `artic-layout`, so neither `artic-text` nor a renderer depends on the other: both speak `artic-layout`'s types. The runner takes the engine as a `Box<dyn TextEngine>`, so `artic-platform` doesn't depend on `artic-text` either; whatever builds the runner, the facade or an example, passes `artic_text::Engine::system()`.

## 2. Value types

In `artic-layout`, beside `Thickness` and `Path`:

```rust
pub struct FontFamily(Arc<[FamilyName]>);
pub enum FamilyName { Named(Arc<str>), Generic(GenericFamily) }
pub enum GenericFamily { Serif, SansSerif, Monospace, Cursive, Fantasy, SystemUi, Emoji, Math }
pub struct FontWeight(u16);
pub enum FontStyle { Normal, Italic, Oblique }

pub struct TextStyle {
  pub family: FontFamily,
  pub size: f32,
  pub weight: FontWeight,
  pub style: FontStyle,
  pub line_height: Option<f32>,
}

pub struct TextLimits {
  pub max_width: Option<f32>,
  pub max_lines: Option<NonZeroU32>,
}

pub struct Paragraph { size: Size, lines: Arc<[Line]>, runs: Arc<[GlyphRun]> }
pub struct Line { pub top: f32, pub height: f32, pub baseline: f32, pub width: f32 }
pub struct GlyphRun { font: FontData, size: f32, coords: Arc<[i16]>, synthesis: Synthesis, glyphs: Arc<[Glyph]> }
pub struct Synthesis { pub embolden: bool, pub skew: f32 }
pub struct Glyph { pub id: u32, pub x: f32, pub y: f32 }
pub struct FontData { bytes: Arc<dyn AsRef<[u8]> + Send + Sync>, index: u32, id: u64 }
```

- `FontFamily: FromStr` reads a CSS family list: names separated by commas, each quoted with `'` or `"` or not. An unquoted name equal to a generic keyword (`serif`, `sans-serif`, `monospace`, `cursive`, `fantasy`, `system-ui`, `emoji`, `math`), ignoring case, is that generic family; a quoted one is a font named so. The default, which `TextBlock` (#46) inherits, is `system-ui`.
- `FontWeight` is 1 to 1000, as in CSS. `FromStr` takes a number or one of WPF's `FontWeights` names, ignoring case: `Thin` 100, `ExtraLight` and `UltraLight` 200, `Light` 300, `Normal` and `Regular` 400, `Medium` 500, `SemiBold` and `DemiBold` 600, `Bold` 700, `ExtraBold` and `UltraBold` 800, `Black` and `Heavy` 900, `ExtraBlack` and `UltraBlack` 950. `FontStyle` is a `XamlEnum`, as `Orientation` is.
- `line_height: None` uses the font's own line spacing. `max_width: None` doesn't wrap, and `max_lines: None` keeps every line. `TextLimits::wrap(width)` and `TextLimits::NONE` are the two common values.
- `Paragraph`, `GlyphRun` and `FontData` keep their fields private and expose them through read-only accessors (`size()`, `lines()`, `runs()`, `font()`, `size()`, `coords()`, `synthesis()`, `glyphs()`, `bytes()`, `index()`, `id()`), and are built through `new` functions, so an engine outside artic can build them too and a renderer can't change a recorded paragraph.
- A `Paragraph` is cheap to clone: its lines and runs are behind `Arc`s. Glyph positions are in logical units, relative to the paragraph's top-left corner, with `y` on the glyph's baseline. Nothing is snapped to device pixels.
- `coords` are the run's variable-font axis positions, in `F2Dot14` form, so a weight drawn from a variable font comes out at that weight.
- `synthesis` is what the font lacks and the renderer fakes: `embolden` when a bold weight was asked of a family without a bold face, and `skew`, in degrees, when italic was asked of one without an italic face. `fontique` reports both when it picks the face.
- `FontData::new(bytes, index)` takes a new `id` from a process-wide counter. Two `FontData` are equal when their `id` and `index` are, never by comparing bytes, so `Paragraph: PartialEq` stays cheap and two recordings of the same text compare equal. `FontData`'s `Debug` is written by hand and shows the id, index and length, since the bytes have no `Debug` of their own. The bytes are the same type `peniko::Blob` holds, so a font `fontique` mapped from disk reaches vello without a copy.

Errors follow `ParseBoxShadowError`: the text and a problem enum.

```rust
pub struct ParseFontFamilyError { pub text: String, pub problem: FamilyProblem }
pub enum FamilyProblem { Empty, EmptyName, UnclosedQuote }

pub struct ParseFontWeightError { pub text: String, pub problem: WeightProblem }
pub enum WeightProblem { UnknownName, OutOfRange }
```

`"Inter,,Arial"` is `EmptyName`, and `"'Segoe UI"` is `UnclosedQuote`. Messages quote the text and show the forms: `invalid FontFamily '': it's empty; expected forms like 'Inter, sans-serif' or 'system-ui'`.

## 3. The `TextEngine` trait

```rust
pub trait TextEngine {
  /// Shapes `text` in `style` and breaks it into lines within `limits`.
  fn paragraph(&mut self, text: &str, style: &TextStyle, limits: TextLimits) -> Paragraph;
}

/// An engine without fonts: every paragraph is empty, of size 0.
pub struct NoText;
impl TextEngine for NoText { /* … */ }

impl MeasureCx<'_> {
  pub fn text(&mut self) -> &mut dyn TextEngine;
}

impl ArrangeCx<'_> {
  pub fn text(&mut self) -> &mut dyn TextEngine;
}
```

The name says what it is, a text engine, and the method names what it returns. "Shaper" would cover only part of the job, "typesetter" reads as something about types in Rust, and "layout" already means measuring and arranging elements in this crate.

A control shapes in `measure` and keeps the `Paragraph` for `render`. `Layoutable::measure` takes `&self`, so the control keeps it in a cell, keyed by the text, style and limits it was shaped with; `TextBlock` (#46) owns that. A parent can measure a child more than once and arrange it narrower than the width it was measured at, so `ArrangeCx` has the engine too, and a control whose final width is narrower than its paragraph shapes it again there. Alignment in a wider box needs no shaping: it offsets each line by its `width`. `RenderCx` doesn't have the engine; render only draws what layout kept.

`NoText` is for layout that has no text to shape: `artic-layout`'s and `artic-element`'s own tests, and anything that runs layout before fonts matter.

`LayoutManager::run` borrows the engine for one pass:

```rust
pub struct Viewport { pub available: Size, pub scale: f32, pub snap: bool }

pub fn run(&mut self, tree: &mut UiTree, nodes: &impl LayoutNodes, root: NodeId, viewport: Viewport,
           text: &mut dyn TextEngine) -> LayoutStats;
pub fn measure(&mut self, tree: &mut UiTree, nodes: &impl LayoutNodes, root: NodeId, available: Size,
               text: &mut dyn TextEngine) -> Size;
```

`LayoutManager::measure`, which measures without arranging, runs controls' `measure` too, so it takes the engine as well. `available`, `scale` and `snap` become one `Viewport`, which also keeps `run` under clippy's argument limit. `Pass` holds the `&mut dyn TextEngine` beside its other borrows, so no control can keep the engine past the pass. Layout runs on the UI thread, so the engine needn't be `Send`, and `parley`'s contexts stay plain `&mut` state.

The runner owns a single engine that every surface shares, so system fonts are listed once and a font keeps one identity in every window. It lends that same engine to each surface's `Pipeline::prepare`, which passes it to `run`, and to `Pipeline::measure`, which it calls before opening a window its content sizes, and which passes it to `LayoutManager::measure`. `Runner::new` takes the engine after the renderer. `artic-test`'s headless layout helper and `render_tree` use the test engine of section 7.

## 4. The engine

In `artic-text`:

```rust
pub struct Engine { /* parley::FontContext, parley::LayoutContext, HashMap<(u64, u32), FontData> */ }

impl Engine {
  /// An engine that finds the system's fonts.
  pub fn system() -> Engine;
  /// An engine that knows only `fonts`, with every generic family pointing at them, in order.
  pub fn with_fonts(fonts: impl IntoIterator<Item = Arc<dyn AsRef<[u8]> + Send + Sync>>) -> Engine;
}

impl TextEngine for Engine { /* … */ }
```

`paragraph`:

1. builds a `parley` layout at display scale 1, without quantizing, so its numbers are logical units. The family list, with `system-ui` added at its end, becomes a `parley` font stack; size, weight and style carry over; `line_height: None` is the font's metrics (`MetricsRelative(1.0)`);
2. breaks it with `break_all_lines(max_width)`, then calls `align` with start alignment, which `parley` needs to place the lines. `parley` reorders mixed-direction runs; alignment modes belong to #46;
3. keeps the first `max_lines` lines. The size is the widest kept line by the sum of the kept lines' heights;
4. turns each `parley` glyph run into a `GlyphRun`, with its synthesis, looking its font up in the map by the blob's id and the face's index, so one face always has one `FontData` and one `id` for the engine's life. Faces of a collection file (`.ttc`) share a blob and differ by index.

It never panics and never fails:

| Input | Result |
| --- | --- |
| A family that isn't installed | the next name in the list, then `system-ui`, then `fontique`'s fallback for the text's script |
| A weight or style the family has no face for | the nearest face, with `synthesis` set |
| Empty text | width 0, and the height of one line, as an empty WPF `TextBlock` |
| A size that isn't finite or isn't above 0, or no font at all | an empty paragraph of size 0 |
| A `max_width` that is NaN or infinite | no wrapping |
| A negative `max_width` | 0, so the text breaks at every opportunity |

The default family, `system-ui`, is mapped to a real font by `fontique`, per desktop. artic keeps no font list of its own, so a new OS release changes nothing here. `with_fonts` turns system fonts off and points every generic family at the given fonts. Since every list ends in `system-ui`, an unknown name such as `"Nope"` lands on them too, so in tests every family resolves and nothing depends on what the machine has installed. `Engine`'s `Debug` is written by hand, since `parley`'s contexts have none worth printing.

## 5. The scene

In `artic-render`:

```rust
pub enum Command {
  // …
  /// Draws `paragraph`'s glyphs with its top-left corner at `origin`, painted with `brush`.
  Text { paragraph: Paragraph, origin: Point, brush: Brush },
}

impl SceneBuilder {
  pub fn text(&mut self, paragraph: &Paragraph, origin: Point, brush: impl Into<Brush>);
}
```

One command draws a whole paragraph. Any `Brush` works, and its relative points and radii resolve against `Rect::new(origin, paragraph.size())`, as a `Fill` resolves against its shape's bounds, so a gradient runs across the whole text rather than restarting with each run. A paragraph without runs records nothing.

A brush per span of text (inline `Run`s, #47) can extend this later with a brush per run, without changing the command's shape.

## 6. Drawing with vello

`artic-vello` draws each run with one `draw_glyphs` call: the run's size and `coords`, the brush, and the transform moved by `origin`. `synthesis` becomes vello's `font_embolden` and a skewing `glyph_transform`. Hinting is off, since it depends on the device-pixel grid, which text doesn't snap to yet.

vello 0.11's renderer already caches glyph outlines between frames, keyed by the font blob's id, its index, the size and the variation coordinates, and drops entries left unused for 64 frames, checking every 64 frames or sooner once it holds more than 256 glyphs. So artic builds no glyph cache. What it must keep is the blob's identity: `artic-vello` maps each `FontData` id and index to one `peniko::FontData`, built once from the same bytes, and reuses it. A blob made anew for each frame would have a new id and miss the cache every time. The map isn't pruned, since a process uses a bounded set of fonts.

Freeing memory on a memory warning (#146) decides then what to drop; vello already evicts old outlines by itself.

## 7. Testing

The test font is a font under the SIL Open Font License, picked in the plan by size, with a regular and a bold face as separate files and no italic face. Bold then selects a real face; italic is synthesized, and so is bold when the engine is given only the regular face. `artic-text` owns the files and their licence, behind a `test-fonts` feature: `Engine::test()` and `Engine::test_regular()` embed them with `include_bytes!`, so nothing outside the feature carries the bytes. `artic-text`'s integration tests declare `required-features = ["test-fonts"]`. `artic-test` enables the feature and its `text::engine()` returns `Engine::test()`; it depends on `artic-text`, never the other way, so there's no cycle and one copy of the font.

| Crate | Tests |
| --- | --- |
| `artic-layout` | `font_family_parses_list` (quoted and unquoted names, generic families), one test per `FamilyProblem` and `WeightProblem`, each checking the message quotes the text; `font_weight_names_and_numbers`; `font_data_compares_by_id`; `measure_reaches_the_text_engine` and `arrange_reaches_the_text_engine` (a fake engine records the calls `run` passes through); `measure_only_reaches_the_text_engine` (through `LayoutManager::measure`); `no_text_gives_empty_paragraphs` |
| `artic-text` | `paragraph_size_with_test_font` (width and height within 0.5 px of the expected values), `line_breaking_respects_max_width` (the line count at two widths), `max_lines_caps_the_paragraph`, `missing_family_falls_back`, `empty_text_has_one_line_height`, `bad_sizes_give_an_empty_paragraph`, `unusable_max_width_does_not_wrap` (NaN and infinity), `negative_max_width_breaks_everywhere`, `line_height_sets_line_spacing`, `lines_stack_top_to_bottom` (each line's `top` is the previous line's `top` plus its `height`, and its `baseline` lies inside it), `bold_selects_a_heavier_face`, `missing_bold_is_synthesized` (only the regular face), `missing_italic_is_synthesized`, `right_to_left_text_stays_inside_its_size`, `same_font_keeps_its_id`, `collection_faces_get_their_own_ids` |
| `artic-render` | `text_records_one_command`, `empty_paragraph_records_nothing` |
| `artic-platform` | the pipeline tests pass an engine; `one_engine_serves_every_surface`; `content_sized_window_measures_text` (through `Pipeline::measure`) |
| `artic-vello` | `one_blob_per_font_across_frames`, `synthesized_bold_draws_heavier`; goldens `text_solid`, `text_gradient`, `text_wrapped`, each `per_os` as the [golden images design](golden-images.md) says for text; a text tile in `examples/primitives` |

## 8. Documents that change

| Document | Change |
| --- | --- |
| `docs/design/frame.md`, `docs/design/surface.md` | the facade's call becomes `Runner::new(tree, renderer, engine, application)` |
| `docs/architecture.md` | `artic-text` in layer 2, the graph and the edge table; the layer 1 role includes text values and the layer 2 role no longer says `artic-render` shapes text; the text-measurement bullet names `artic-text` as the trait's implementation |
| Spec §2 | the crate table: `artic-render` no longer shapes text, and `artic-text` gets a row |
| Spec §10 | the Text bullet: shaping is in `artic-text`, not `artic-render` |
| `docs/design/backends.md` | the crate table and the list of changes say text shaping is in `artic-text` |
| `docs/design/border.md` | §6: snapping chrome to device pixels is #295 |
| Issue #37 | the entry point is `TextEngine`, not `Renderer::measure_text`; `measure_text_with_test_font` becomes `paragraph_size_with_test_font`; the glyph cache criterion becomes `one_blob_per_font_across_frames` |

## 9. What waits

| Feature | How it fits | Tracked in |
| --- | --- | --- |
| Wrapping and trimming modes, alignment, the inherited font and foreground properties | `TextBlock` declares them and fills `TextStyle` and `TextLimits` | #46 |
| Inline runs with their own brush, weight and style | spans in `paragraph`, and a brush per run in `Command::Text` | #47 |
| Color emoji | vello's `COLRv1` glyphs | #175 |
| App fonts from resources | `Engine` registers them beside the system's | a later issue |
| Outlines for CPU renderers | a `skrifa` helper, as the [backends design](backends.md) says | the first CPU renderer |
| Snapping text and hinting | the device-pixel grid in `RenderCx` | with #295 |
| Freeing memory on a warning | whatever #146 decides to drop | #146 |

## 10. Pull requests

| PR | Contents | Done when |
| --- | --- | --- |
| A | this document and the documents in section 8 | approved |
| B | sections 2 and 3: the value types and parsers, `TextEngine`, `NoText`, `MeasureCx::text()` and `ArrangeCx::text()`, `Viewport`, and the engine through `LayoutManager::run` and `measure`, `Pipeline::prepare` and `measure`, and the runner; every caller passes `NoText` for now | the `artic-layout` and `artic-platform` tests in section 7 pass |
| C | section 4: `artic-text`, with `parley` and `fontique` pinned exactly in the workspace beside `vello`; the test font, its licence and the `test-fonts` feature; `artic_test::text::engine()`, which `artic-test`'s helpers pass instead of `NoText`; `Engine::system()` in the `frames` example | the `artic-text` tests pass |
| D | sections 5 and 6: `Command::Text`, `SceneBuilder::text`, vello drawing, the goldens and the example; closes #37 | the `artic-render` and `artic-vello` tests pass |
