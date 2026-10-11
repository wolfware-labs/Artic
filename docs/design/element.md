# Design: elements

Issue: #42. Implemented by four pull requests under #42, in order: A (`artic-element`), B (`ElementBase`), C (`#[derive(Element)]`), D (accessibility and composition). Related: #41 (layout), #33 (scene composition), #84 (`ContentState`, `TemplateState`), #241 (`linkme` on wasm).

Every control has the same body: a size, a margin, an alignment, a visibility, an opacity. WPF gives it to them through a base class, `FrameworkElement`. Rust has no class inheritance, so artic gives it through composition: a control embeds an `ElementBase`, and `#[derive(Element)]` writes the code that connects the two. This document fixes the traits, the base and what the derive writes.

## 1. Pieces

| Piece | Crate | What it is |
| --- | --- | --- |
| `Element` | `artic-element` | The trait the frame pipeline drives: layout, render, hit testing, input, accessibility |
| `ElementImpl` | `artic-element` | The trait a control author implements: only what is special about the control |
| `HasState<S>` | `artic-element` | Gives a control's state structs to the traits built on them |
| `ElementType` | `artic-element` | How the pipeline finds a node's `&dyn Element` |
| `HitTestCx`, `EventCx`, `RenderCx`, `AccessCx`, `HitTestResult`, `Handled` | `artic-element` | Placeholders M3 fills in, enough for the defaults |
| `InputEvent` | `artic-core` | Placeholder input data, named to avoid `Event<A>`, the field type |
| `ElementBase`, `Visibility`, `Cursor` | `artic-controls` | The universal properties |
| `#[derive(Element)]`, `#[derive(XamlProperties)]` | `artic-macros` | The two new derives |

## 2. The traits

```rust
pub trait Element: Object + Layoutable {
  fn render(&self, cx: &mut RenderCx<'_>, scene: &mut SceneBuilder);
  fn hit_test(&self, cx: &HitTestCx<'_>, point: Point) -> HitTestResult { cx.bounds_test(point) }
  fn on_event(&mut self, cx: &mut EventCx<'_>, event: &InputEvent) -> Handled { Handled::No }
  fn accessibility(&self, cx: &AccessCx<'_>) -> accesskit::Node;
  /// The node's layers in composition for its arranged `size`: transform, clip, opacity,
  /// visibility, z-index.
  fn layers(&self, size: Size) -> Layers { Layers::default() }
}

pub trait ElementImpl {
  /// The content's size within `available`, which already excludes the margin and is held within
  /// the element's limits.
  fn measure_override(&self, cx: &mut LayoutCx<'_>, available: Size) -> Size { Size::ZERO }
  /// Places the children within `size`, in the element's own coordinates.
  fn arrange_override(&self, cx: &mut LayoutCx<'_>, size: Size) {}
  fn render(&self, cx: &mut RenderCx<'_>, scene: &mut SceneBuilder) {}
  fn on_event(&mut self, cx: &mut EventCx<'_>, event: &InputEvent) -> Handled { Handled::No }
  /// Adds the control's role, name, value and actions to the default node `ElementBase` built.
  fn accessibility(&self, cx: &AccessCx<'_>, node: &mut accesskit::Node) {}
  /// Clips the element and its descendants, in its own coordinates, for its arranged `size`.
  fn clip(&self, size: Size) -> Option<Clip> { None }
  // #84 adds, with TemplateCx: fn on_apply_template(&mut self, cx: &mut TemplateCx<'_>) {}
}
```

`ElementImpl` is also where every per-control hook goes, each with a default: `on_apply_template` joins it rather than living on `Templated`, because state traits come from blanket impls (section 3), which a control can't override. #84 adds the hook together with `TemplateCx`, since nothing applies templates before it; PR A has no placeholder for either.

- The supertrait is `Object`, the trait in `artic-core`; spec §7's `XamlObject` is renamed to match. Not every node comes from markup: builder-made and test nodes implement `Object` too.
- `Layoutable` (#41) brings `measure` and `arrange`, so `Element` adds only what layout doesn't need.
- `Element` is what the pipeline calls; `ElementImpl` is what the author writes. The derive connects them, so an author can't skip the shared work by overriding the wrong method. A control with no content writes `impl ElementImpl for Spacer {}`.

## 3. What the derive writes

`#[derive(Element)]` requires a field `base: ElementBase` and writes `Layoutable` and `Element` for the struct:

| Method | Shared work, from `ElementBase` | Then calls |
| --- | --- | --- |
| `measure` | `Collapsed`: returns zero, children not measured. Otherwise #41's `measure_with`: margin off, limits on | `measure_override` |
| `arrange` | `Collapsed`: returns, children not arranged, since `arrange_with` would stretch a zero size to the slot. Otherwise #41's `arrange_with`: margin, limits, alignment | `arrange_override` with the content's size |
| `render` | | `ElementImpl::render`; a hidden subtree is skipped by composition (section 7) |
| `hit_test` | not `Visible`, or `IsHitTestVisible` false: a miss. Otherwise the trait default, `cx.bounds_test(point)` | |
| `on_event` | `IsEnabled` false (the inherited value): `Handled::No`, the control isn't called | `ElementImpl::on_event` |
| `accessibility` | builds the default node (section 7) | `ElementImpl::accessibility`, which adds to it |
| `layers` | `Opacity` (clamped), `RenderTransform`, `Visibility` and `ZIndex` as a `Layers` (section 7) | `ElementImpl::clip`, for the clip layer |

A struct without `base: ElementBase` fails to compile with an error that names the missing field.

### State structs

For a field marked `#[element(state)]`, the derive implements `HasState<S>` for the struct, where `S` is the field's type:

```rust
pub trait HasState<S> {
  fn state(&self) -> &S;
  fn state_mut(&mut self) -> &mut S;
}

#[derive(Debug, Element, XamlType)]
pub struct Button {
  #[xaml(flatten)] pub base: ElementBase,
  #[element(state)] #[xaml(flatten)] pub content: ContentState, // HasState<ContentState>; its properties are Button's
}

impl ElementImpl for Button { /* measure_override, render, on_event, … */ }

// next to ContentState, in artic-controls (#84):
pub trait HasContent: Element + HasState<ContentState> {
  fn content(&self) -> &Property<Content> {
    &HasState::<ContentState>::state(self).content
  }
}
impl<T: Element + HasState<ContentState>> HasContent for T {}
```

The rule, as documented on the derive: for each `#[element(state)]` field of type `S`, the derive implements `HasState<S>` for the struct, and the traits built on a state are implemented through `HasState<S>` by a blanket impl. A state trait therefore holds only methods its state fully determines, each with a default that reads the state; it names the state type (`HasState::<ContentState>::state(self)`) because a struct can have several. A blanket impl can't be overridden for one type, so anything a control customizes is an `ElementImpl` hook instead (section 2). A state struct that holds properties, such as `ContentState`'s `Property<Content>`, is also a property group: it derives `XamlProperties`, and its field is marked `#[xaml(flatten)]` too, so its properties are registered and visited like `ElementBase`'s. The two attributes are read by different derives: `element(state)` by `Element`, `xaml(flatten)` by `XamlType`. A struct has at most one field per state type, since two would mean two `impl HasState<S>` for the same `S`. The derive reports two fields whose types are written the same way, naming both; two names for one type, such as an alias, reach rustc's conflicting-implementations error (E0119) instead, which the rule's rustdoc explains. There is no table of type names in the macro, so aliases work, and any crate, apps included, can add a state struct and its trait without changing `artic-macros`.

## 4. Finding a node's `Element`

`UiTree` stores `Box<dyn Object>`, and Rust can't downcast a `&dyn Object` to a trait `artic-core` doesn't know. `#[derive(Element)]` therefore adds the type to a `linkme` slice in `artic-element`:

```rust
pub struct ElementType {
  pub type_id: fn() -> TypeId,
  pub cast: fn(&dyn Object) -> Option<&dyn Element>,
  pub cast_mut: fn(&mut dyn Object) -> Option<&mut dyn Element>,
}

// emitted for Border; downcasting is defined on dyn Any, so the object is upcast first, as
// UiTree::get_as does. Generated code names every item by its full path, since `Any` isn't in the
// prelude:
cast: |object| (object as &dyn ::core::any::Any).downcast_ref::<Border>().map(|border| border as &dyn Element),
cast_mut: |object| (object as &mut dyn ::core::any::Any).downcast_mut::<Border>().map(|border| border as &mut dyn Element),
```

The pipeline builds a `TypeId → ElementType` map once. `cast` answers `LayoutNodes` and the render `Nodes`; `cast_mut` serves input dispatch, which calls `on_event(&mut self)`. A hand-written `Element` registers with `element_type!(TestElement)`, the line the derive emits. App controls need no registration call, as spec §7 requires. `linkme` doesn't support wasm yet; the registry has the same limit, and #241 fixes both.

## 5. `ElementBase`

`ElementBase` is a property group, not a XAML type: `#[derive(XamlProperties)]` gives it its keys, `PROPERTIES`, `visit_properties` and `Default`, and markup can't create one. Its keys are generated once, so every control shares them, and `IsEnabled` inherits across control types.

| Property | Type | Default | Affects | Inherits |
| --- | --- | --- | --- | --- |
| `Width`, `Height` | `Option<f32>` | `None` (size to content) | layout | |
| `MinWidth`, `MinHeight` | `f32` | `0` | layout | |
| `MaxWidth`, `MaxHeight` | `f32` | infinity | layout | |
| `Margin` | `Thickness` | zero | layout | |
| `HorizontalAlignment`, `VerticalAlignment` | the #41 enums | `Stretch` | layout | |
| `Visibility` | `Visibility` | `Visible` | layout, render | |
| `IsEnabled` | `bool` | `true` | render | yes |
| `Opacity` | `f32`; the property keeps any value, and composition clamps it (section 7) | `1` | render | |
| `RenderTransform` | `Option<Transform>` | `None` | render | |
| `ZIndex` | `i32`: draw and hit-test order among siblings | `0` | render | |
| `Classes` | `Classes` (space-separated names) | empty | style, through its signal (#107) | |
| `Name` | `Option<String>` | `None` | style, through its signal (#107) | |
| `DataContext` | `Option<NodeId>` (a view-model node) | `None` | | yes |
| `Cursor` | `Cursor` | `Arrow` | | |
| `Focusable` | `bool` | `false` | render (clears cached hit tests: touch inflates focusable targets, §12) | |
| `IsHitTestVisible` | `bool` | `true` | render (clears cached hit tests, §12) | |
| `ToolTip` | `Option<String>` | `None` | | |

The size properties and the alignments are what #41's `Sizing` holds; `ElementBase` builds a `Sizing` from them for `measure_with` and `arrange_with`.

`Visibility`, `Cursor` and `Classes` live in `artic-controls`, with `#[derive(XamlEnum)]` for the enums. The alignment enums already live in `artic-layout`, which is on the registry's layer (1) and so can't depend on it, nor on `artic-macros`'s registry output; they get `FromStr` the way `Thickness` does.

### Visibility

There is one visibility property, `Visibility`, with three states. Two settable properties for one fact would need a rule for when they disagree, so there is no `IsVisible` beside it.

The three states are what web and mobile developers already know: CSS `visibility: hidden` and `display: none`, Android's `VISIBLE` / `INVISIBLE` / `GONE`, Flutter's `Visibility`. A boolean `IsVisible`, as in Avalonia, was considered and rejected: "hidden but keeps its space" would then be `Opacity = 0`, which still takes clicks and is still read by screen readers unless `IsHitTestVisible` and an accessibility setting are changed too. `Hidden` does all three. This reverses the earlier naming review (#239); see `naming-review.md`.

| `Visibility` | Takes space | Drawn | Hit-testable | accesskit |
| --- | --- | --- | --- | --- |
| `Visible` | yes | yes | yes | normal |
| `Hidden` | yes | no | no | not emitted |
| `Collapsed` | no: desired size zero, children not measured | no | no | not emitted |

Whether an element and all its ancestors are visible is computed when needed, for example by a future `is_visible(tree, node)`, never set.

`LayoutManager` (#41) needs one change for `Collapsed` to hold for a whole subtree. Its upward pass measures dirty nodes on their own, and its waiting pass arranges nodes from their cached rectangles, so a child laid out before its parent collapsed would still be reached through its cached results. The manager therefore forgets what it knew about the children a node's `measure` didn't measure, together with their subtrees; a forgotten node has no cached size or rectangle, so both passes skip it until its parent measures it again. `artic-layout` still never names `Visibility`: "didn't measure this child" is all it sees.

## 6. Registry: flattened groups

`#[derive(XamlType)]` skips fields without `#[xaml]`, so a plain `base: ElementBase` would be missing from the registry and from `visit_properties`: `Margin` couldn't be set from markup, changing it wouldn't mark the node dirty, and `IsEnabled` wouldn't inherit. A control marks the field:

```rust
#[derive(Debug, Element, XamlType)]
#[xaml(namespace = "https://artic.rs/ui", content = child)]
pub struct Border {
  #[xaml(flatten)] pub base: ElementBase,
  #[xaml] pub child: Property<Option<NodeId>>,
}
```

For a flattened field, `derive(XamlType)`:

- calls `self.base.visit_properties(visit)` from the struct's own `visit_properties`;
- implements `Flattened<ElementBase>` for the struct, whose one method returns `&self.base`;
- adds `PropertyGroupInfo { field: "base", properties: GroupOf::<ElementBase, Border>::PROPERTIES }` to the type's `TypeInfo.groups`.

The group's descriptors are generated once per owner, through generics. The loader's thunks ([loader values](loader-values.md)) take a tree and a node and downcast the node to the property's owner, so one shared `ElementBase` thunk couldn't reach a `Border`'s margin. `#[derive(XamlProperties)]` therefore writes each thunk generic over the owner, and the descriptor list as an associated `const` of a generic type, which the compiler instantiates for each control:

```rust
// generated by #[derive(XamlProperties)] on ElementBase:
fn margin_set_text<O: Object + Flattened<ElementBase>>(
  tree: &mut UiTree, node: NodeId, text: &str, level: Precedence,
) -> Result<(), ValueError> {
  let value: Thickness = text.parse().map_err(/* … */)?;
  let owner = tree.get_as::<O>(node).ok_or(ValueError::WrongNode)?;
  owner.group().margin.set_at(level, value);
  Ok(())
}

impl<O: Object + Flattened<ElementBase>> GroupOf<ElementBase, O> {
  pub const PROPERTIES: &'static [PropertyInfo] = &[ /* Margin's access uses margin_set_text::<O>, … */ ];
}
```

The loader's contract doesn't change: a flattened property's thunks have the same signatures and downcast to the control, like the control's own.

Lookups search the type's own properties, then each group, and the compiler writes `node.base.margin.set(...)`. A name must be unique across a type and all its groups: a control property named `Margin` would otherwise hide `ElementBase`'s, so markup would set a value that layout never reads. The derive can't see a group's names, which come from another crate, so the registry checks when it's built and reports a clash as a new `Duplicate::Property`, as it already does for duplicate type names. A slice of groups, not one merged list, because a `const` can't join two slices whose lengths come from different crates. Spec §3's `Border` gains the attribute.

## 7. Accessibility and composition

`ElementBase` builds the default accesskit node: the arranged bounds, and disabled when `IsEnabled` is false. Only `Visible` elements get a node at all: like composition and hit testing, the accesskit walk skips a node that isn't `Visible` together with its whole subtree, so a `Visible` child of a hidden parent is never exposed. The tree is updated from render-dirty nodes, and a change of `Visibility` marks only the node that changed, so a node that becomes `Visible` again has its whole accessible subtree rebuilt, not just its own node; otherwise its unchanged descendants would stay missing. Layout moves nodes without marking them render-dirty, so the update also covers every node `LayoutManager` gave a new rectangle. Each node's bounds are in its parent's coordinates, placed with accesskit's per-node transform, so a moved node updates alone and its descendants don't change. Each control adds its role, name, value and actions in `ElementImpl::accessibility`, which the derive calls with that node (#161 adds the attached properties).

`Opacity`, `RenderTransform`, `Visibility` and `ZIndex` feed #33's per-node layers, not each control's `render`. The derive writes `Element::layers` from `ElementBase`, and the pipeline's render `Nodes` answers `layers(tree, node)` by casting the node with `ElementType::cast` and calling it with the node's arranged size from `LayoutManager`, so app-defined controls work the same way: opacity becomes the node's opacity layer, the transform its transform layer, and a control's `ElementImpl::clip`, such as a scroll viewer's viewport, its clip layer. Visibility needs a layer of its own: today the compositor's traversal, the private `Walk::visit` in `compose.rs`, always visits a node's visual children, so hiding only the node's own drawing would still draw its children. `Layers` gains `visible: bool`, default `true`, and `visit` skips a node with `visible: false` together with its whole subtree. `Hidden` and `Collapsed` answer `visible: false`.

`Opacity` is a plain `f32`, so a Rust write can set any value, while `Layers.opacity` must be from 0 to 1. The property itself keeps whatever was set, and reading it returns that value. Only `ElementBase`'s building of the node's `Layers` clamps it to 0–1, with NaN counting as 1, so composition never sees an invalid value.

`ZIndex` orders siblings. `Layers` gains `z_index: i32`, default `0`, and `visit` draws a node's children sorted by it, stably, so equal values keep tree order. The hit-test walk (#74) uses the same order, reversed, so what is hit is what is drawn on top.

`Classes` and `Name` decide which style selectors match (§8) and have no direct effect on layout or rendering. Like every property, each is a signal, and the style index (#107) learns of changes by depending on them and re-evaluates the selectors, so they need no `Affects` flag. A style that starts or stops matching changes properties such as `Width` or `Opacity`, and those changes carry their own invalidation.

Hit testing follows the same rule: the derive's `hit_test` misses when the element isn't `Visible` or its `IsHitTestVisible` is false, and the hit-test walk (#74) doesn't descend into such a node. `IsHitTestVisible` is the drawn-but-click-through case that `Visibility` can't express, for overlays and decorations; spec §12 relies on it.

A disabled element is different: it gets no input, but it still blocks it. Hit testing finds it as usual, so a click on a disabled button lands on the button and doesn't fall through to what's behind it, as with HTML's `<button disabled>`. The derive's `on_event` then returns `Handled::No` without calling `ElementImpl::on_event` when `IsEnabled` is false, so no control has to check it. `IsEnabled` inherits, so a disabled panel disables everything inside it, for pointer, keyboard and every other input. Focus navigation (#78) skips disabled elements. Tooltips come from the pipeline's hover tracking rather than `on_event`, so a disabled control can still show one.

## 8. Pull requests

| PR | Contents | Done when |
| --- | --- | --- |
| A | `artic-element`: `Element`, `ElementImpl`, `HasState`, `ElementType` and `element_type!`, the placeholder contexts; `InputEvent` in `artic-core` | a hand-written element is laid out through `LayoutManager` by way of the cast table |
| B | `ElementBase` with `#[derive(XamlProperties)]`, the enums, `#[xaml(flatten)]` in `derive(XamlType)` with `Flattened` and the per-owner `GroupOf` descriptors, `PropertyGroupInfo`, `Duplicate::Property`, the measure and arrange outline, `LayoutManager` forgetting unmeasured children | `element_base_properties_registered`, `margin_and_alignment_layout`, `min_max_clamp_desired_size`, `collapsed_takes_no_space` (including a child made dirty after its parent collapsed), a registry test where a control property clashes with a group's |
| C | `#[derive(Element)]`: the outline from section 3, the missing-`base` error, `#[element(state)]` and its duplicate-state error | the trybuild cases: missing `base`, two fields of one state type, and a test-only state giving a test trait through a blanket impl; a unit test that a disabled element's hook isn't called |
| D | the default accesskit node; opacity, transform and visibility in composition, with `Layers.visible` | `base_accessibility_node`; a pixel test where opacity changes the drawn color; a hidden parent's child isn't drawn and isn't in the accesskit tree, and is back in both once the parent is `Visible` again; a layout-only change, such as a new `Margin`, updates the node's accesskit bounds; siblings drawn by `ZIndex`, ties in tree order; an out-of-range opacity clamped |

## 9. Spec changes

- §7: `Element: Object + Layoutable` instead of `Element: XamlObject`; `ElementImpl` and `HasState` added; `IsVisible` replaced by `Visibility` and its table.
- §3: `Border`'s `base` field gains `#[xaml(flatten)]`; `TypeInfo` gains `groups`.
