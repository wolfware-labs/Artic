# Architecture

artic is a Cargo workspace of layered crates. This document is the reference for which crate may depend on which. The design behind each crate is in the [technical spec](spec/technical_spec.md).

## Layers

A crate depends only on crates in lower layers. Crates in the same layer never depend on each other.

| Layer | Crates | Role |
| --- | --- | --- |
| 0 | `artic-core`, `artic-macros`, `artic-xaml` | Foundations with no artic dependencies: the reactive core and tree, the proc macros, and the XAML parser |
| 1 | `artic-registry`, `artic-layout`, `artic-i18n` | Type metadata, geometry and layout, localization |
| 2 | `artic-render`, `artic-style`, `artic-xaml-compiler`, `artic-xaml-runtime` | The `Renderer` seam, scene and text shaping; styling; the two XAML back ends |
| 3 | `artic-element`, `artic-lsp` | The `Element` trait and its contexts; the language server |
| 4 | `artic-controls`, `artic-platform`, `artic-cli` | Controls, the `Host` seam and frame pipeline, and the CLI |
| 5 | `artic-theme`, `artic-test`, `artic-winit`, `artic-vello` | The default theme, test utilities, and the default backend adapters |
| 6 | `artic` | The facade that apps depend on |

## Dependency graph

Arrows point from a crate to its dependency.

```mermaid
flowchart TD
  artic --> theme[artic-theme]
  artic --> controls[artic-controls]
  artic --> platform[artic-platform]
  artic -. hot-reload / runtime-xaml .-> runtime[artic-xaml-runtime]

  theme --> controls
  theme --> style[artic-style]

  controls --> element[artic-element]
  controls --> style

  platform --> element

  element --> render[artic-render]
  element --> registry[artic-registry]
  element --> macros[artic-macros]

  test[artic-test] --> platform
  test --> style

  winit[artic-winit] --> platform
  vello[artic-vello] --> render

  cli[artic-cli] --> lsp[artic-lsp]
  lsp --> compiler[artic-xaml-compiler]

  render --> layout[artic-layout]
  style --> registry
  style --> xaml
  compiler --> registry
  compiler --> xaml[artic-xaml]
  runtime --> registry
  runtime --> xaml

  registry --> core[artic-core]
  layout --> core
  i18n[artic-i18n] --> core
```

The graph omits edges that a lower edge already implies (for example `artic-controls -> artic-core`). The full list of direct dependencies:

| Crate | Depends on |
| --- | --- |
| `artic-core` | none |
| `artic-macros` | none (proc-macro; generated code names `artic::__private` or `artic_registry::__private`, whichever the calling crate depends on) |
| `artic-xaml` | none |
| `artic-registry` | `artic-core` |
| `artic-layout` | `artic-core` |
| `artic-i18n` | `artic-core` |
| `artic-render` | `artic-core`, `artic-layout` |
| `artic-style` | `artic-core`, `artic-registry`, `artic-xaml` (for `Span`/`Diagnostic` in selector errors) |
| `artic-xaml-compiler` | `artic-xaml`, `artic-registry` |
| `artic-xaml-runtime` | `artic-xaml`, `artic-registry`, `artic-core` |
| `artic-element` | `artic-core`, `artic-registry`, `artic-layout`, `artic-render`, `artic-macros` |
| `artic-controls` | `artic-core`, `artic-registry`, `artic-layout`, `artic-render`, `artic-element`, `artic-style`, `artic-macros` |
| `artic-platform` | `artic-core`, `artic-layout`, `artic-render`, `artic-element` |
| `artic-cli` | `artic-xaml`, `artic-xaml-compiler`, `artic-lsp` (shared source index for `check` and `lsp`) |
| `artic-lsp` | `artic-xaml`, `artic-xaml-compiler` |
| `artic-theme` | `artic-controls`, `artic-style` |
| `artic-test` | `artic-core`, `artic-registry`, `artic-layout`, `artic-render`, `artic-element`, `artic-style`, `artic-platform` |
| `artic-winit` | `artic-core`, `artic-render`, `artic-platform` |
| `artic-vello` | `artic-layout`, `artic-render` |
| `artic` | every app-facing crate; `artic-xaml-runtime` only with the `hot-reload` or `runtime-xaml` feature |

## Rules

- `artic-controls` does not depend on `artic-platform`. Input event types live in `artic-core`, and the facade connects the platform host to the tree. Controls stay testable without a window and portable to future targets.
- The `Element` trait and its contexts (`RenderCx`, `HitTestCx`, `EventCx`, `AccessCx`) live in `artic-element`, together with gesture recognizers. `WindowDesc` is plain data in `artic-core`, so the platform host and the `Window` control both see it. Controls implement `Element`; the platform host drives any `dyn Element` without knowing the concrete controls. The property groups every control has (`LayoutProperties`, `AppearanceProperties`, `InteractionProperties`, `IdentityProperties`) live there too, so a control library outside artic doesn't need `artic-controls` and its built-in controls; `#[element]` stays with the macros.
- `artic-layout` defines `Layoutable`, `LayoutCx` and `LayoutManager` over `Layoutable`; `Element` extends `Layoutable`, so layout never names `Element`. Text measurement reaches layout through a small trait defined in `artic-layout` and implemented by `artic-render`'s text shaping (`parley`), the same for every renderer.
- `artic-platform` owns the frame pipeline (dispatcher, style pass, layout, scene build, render, accesskit). Stages that need crates the platform cannot see, such as the style pass or OS theme → root pseudo-classes, are registered as hooks by the facade. The platform never depends on `artic-style`, `artic-controls` or `artic-theme`.
- Tree behaviours that call `Element` (routed-event dispatch, hit testing, focus navigation) live in `artic-element`; `artic-core` keeps only the data types they use.
- OS services that controls use (clipboard, IME requests, cursor) are traits in `artic-core`. Hosts provide them as `Host` capabilities, the facade registers them, and controls reach them through their context or app services, never through the `Host`.
- Code that needs concrete controls but serves the runtime or tooling (hot-reload overlay and state restore, debug overlay) lives in the facade.
- Dev-dependencies may point from any crate to `artic-test`, as long as `artic-test` does not depend back on that crate. Control, theme and language-server tests use `TestHost` this way.
- The crates under `crates/artic-registry/fixtures/` are test-only (`publish = false`) and sit outside the layers. Two register one XAML type each; `artic-registry-linking` holds the tests that check which of them reach the registry.
- `artic-macros`' own tests dev-depend on `artic-core` and `artic-registry`, the crates its generated code names. Nothing in those crates depends on `artic-macros` at build time, so this makes no cycle.
- `artic-xaml` does not depend on the registry. Type references stay unresolved in the infoset, so parser tests need no registry.
- Release builds link `artic-xaml-runtime` only when the app enables `runtime-xaml`. Compiled views never touch the loader.
- App-facing crates stay free of native-only dependencies, so a web target can be added later without app changes (D24 in the spec).
- Backend adapters (`artic-winit`, `artic-vello`, and any third-party adapter) depend only on the public API of `artic-render` and `artic-platform`. Nothing in artic depends on an adapter: each target's launcher names its host and renderer ([backend design](design/backends.md)). `artic-render` and `artic-platform` have no backend dependencies (`wgpu`, `vello`, `winit` and the OS crates live in adapters).

## Why this differs from the original §2 diagram

The first version of the spec drew `artic-core -> artic-layout -> artic-controls -> artic-style/platform/render`. Read with the stated arrow direction, that makes the core depend on layout and layout depend on controls, which contradicts "each layer depends only on the ones below it". The graph above is the corrected version, and the spec now points here.

## Checking the graph

```sh
cargo tree -e normal --workspace --depth 1
```

Every crate's direct dependencies must match the table above. A pull request that adds an edge updates this document in the same change.
