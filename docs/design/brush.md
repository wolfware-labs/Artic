# Design: colors and brushes

Issue: #35. Builds on: [value objects](value-objects.md) (#32), the [frame pipeline](frame.md) (#40), geometry parsing (#22). Unblocks: #36 (shapes, clips and shadows), #46 (`TextBlock`), #37, #116.

A brush is the paint behind `Background="#3B82F6"`, `BorderBrush="Red"` or a gradient. This design gives `Color` its XAML text forms, adds `Brush` with solid and gradient variants, and carries brushes through the scene to vello, so a control's `Background` reaches pixels.

```xml
<Border Background="#3B82F6" BorderBrush="SteelBlue"/>
```

```rust
builder.fill_rect(bounds, Color::STEEL_BLUE);
builder.fill_rect(bounds, LinearGradientBrush::new(
  RelativePoint::relative(0.0, 0.0),
  RelativePoint::relative(1.0, 0.0),
  [GradientStop::new(0.0, Color::WHITE), GradientStop::new(1.0, Color::STEEL_BLUE)],
));
```

## 1. Colors

`Color` stays in `artic-render`: sRGB, straight (not premultiplied) alpha, 8 bits per channel. It gains `FromStr`:

| Text | Meaning |
| --- | --- |
| `#RGB` | each digit doubled, opaque: `#3AF` is `#33AAFF` |
| `#ARGB` | each digit doubled, alpha first |
| `#RRGGBB` | opaque |
| `#AARRGGBB` | alpha first, as in WPF and Avalonia |
| a name | the CSS named colors (the list WPF and Avalonia share), plus `transparent`, ignoring ASCII case |

CSS writes 8-digit hex with alpha *last* (`#RRGGBBAA`); the rustdoc says so, since text copied from CSS parses as a different color. The names live in `color/names.rs` as constants (`Color::STEEL_BLUE`) and a lookup table.

A failure is a `ParseColorError { type_name, text, problem, forms }`. Its message is worded like `ParseGeometryError`'s: it repeats the text, says what is wrong, and lists the accepted forms:

```text
invalid Color '#12': it has 2 hex digits; expected "#RGB", "#ARGB", "#RRGGBB", "#AARRGGBB" or a color name
invalid Color 'redd': 'redd' is not a color name; expected "#RGB", "#ARGB", "#RRGGBB", "#AARRGGBB" or a color name
```

## 2. Brushes

One type per markup element, and `Brush` as the family a property holds:

```rust
pub enum Brush {
  Solid(SolidColorBrush),
  Linear(LinearGradientBrush),
  Radial(RadialGradientBrush),
}

pub struct SolidColorBrush { pub color: Color, pub opacity: f32 }
pub struct LinearGradientBrush { pub start: RelativePoint, pub end: RelativePoint, pub stops: Arc<[GradientStop]>, pub opacity: f32 }
pub struct RadialGradientBrush {
  pub center: RelativePoint,
  pub radius_x: RelativeScalar,
  pub radius_y: RelativeScalar,
  pub stops: Arc<[GradientStop]>,
  pub opacity: f32,
}
pub struct GradientStop { pub offset: f32, pub color: Color }
```

| Rule | Why |
| --- | --- |
| Each struct converts into `Brush` with `From`, and so does `Color` (a solid brush) | `fill_rect(rect, Color::RED)` and `Background = gradient.into()` read naturally |
| `FromStr for Brush` parses color text into a solid brush | `Background="Red"` is the common case |
| Defaults: linear from `0%,0%` to `100%,100%`; radial centered at `50%,50%` with radii of `50%`; opacity `1` | A gradient written with only its stops fills its box |
| `opacity` multiplies the alpha of the color, or of every stop | One brush-wide fade, as in WPF and Avalonia, without rewriting the stops |
| Stops are an `Arc<[GradientStop]>` | A brush is cloned into each draw command; a shared slice keeps that cheap |
| Two radii | A radial gradient fills a box that isn't square as an ellipse, as in Avalonia |

The structs have the shape the [value objects](value-objects.md) design gives value elements, so they become `#[xaml(value)]` types with no change when that lands.

## 3. Relative units

Gradient points and radii are resolved against the box being filled, so one brush fits any size. The types are geometry, so they live in `artic_layout::geometry` beside `Point`, where the number parser and `ParseGeometryError` they reuse already are:

```rust
pub enum RelativeUnit { Relative, Absolute }
pub struct RelativePoint { pub point: Point, pub unit: RelativeUnit }
pub struct RelativeScalar { pub value: f32, pub unit: RelativeUnit }
```

`"50%,25%"` is relative, the fractions `0.5, 0.25` of the box; `"10,20"` is absolute, in logical units from the box's top-left corner. A point mixing the two, such as `"50%,20"`, is an error. The unit is visible in the text, so there is no separate mapping mode to remember. Parse errors are `ParseGeometryError`s, with a new problem, `MixedUnits`.

## 4. Drawing

`Command::FillRect` carries a brush instead of a color:

```rust
FillRect { rect: Rect, brush: Brush }
```

`SceneBuilder::fill_rect(rect, brush: impl Into<Brush>)` takes anything that converts, so callers that pass a `Color` today keep compiling.

`artic-render` stays free of any backend. `artic-vello` converts, in `translate.rs`:

- Relative points and radii resolve against the command's `rect`.
- A linear gradient becomes a `peniko` linear gradient between the resolved points.
- A radial gradient becomes a circular `peniko` gradient of radius 1 at the origin, with a brush transform that scales it by the two radii and moves it to the center, so unequal radii draw an ellipse.
- `opacity` multiplies the alpha of the color or of each stop.

## 5. What waits

`<SolidColorBrush Color="#3B82F6"/>`, `<LinearGradientBrush>` with its `GradientStop`s, and `<Border.Background>` property elements need the value-object machinery (`#[xaml(value)]`, `TypeKind`, `FromXamlObject`), which #57 implements with #43. Until then, markup sets brushes as color text, and Rust code builds gradients directly. A gradient spread mode (pad, reflect, repeat) and a gradient origin apart from the center are additive and wait for a need.

## 6. Testing

| Crate | Tests |
| --- | --- |
| `artic-layout` | `relative_point_parses_percent_and_absolute`, `relative_point_rejects_mixed_units` |
| `artic-render` | `color_parses_short_hex` (`#RGB`, `#ARGB`), `color_parses_rrggbb`, `color_parses_aarrggbb`, `color_parses_named_case_insensitive`, `color_rejects_invalid` (bad length, a non-hex digit, an unknown name, each message naming the input), `brush_from_color_string_is_solid` |
| `artic-vello` | `brush_to_peniko_roundtrip` (color, stops, resolved points, opacity), `linear_gradient_fills_rect` (pixels: the left edge is the first stop, the right edge the last) |

## 7. Pull requests

| PR | Contents | Done when |
| --- | --- | --- |
| A | this document | approved |
| B | relative units in `artic-layout`; `Color` parsing and names, `Brush` and its structs, `FillRect` with a brush in `artic-render`; the vello conversion | the tests in section 6 |
