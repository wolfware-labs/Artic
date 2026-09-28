# Architecture

artic is a Cargo workspace of layered crates. This document is the reference for which crate may depend on which. The design behind each crate is in the [technical spec](spec/technical_spec.md).

## <a id="arch-layers"></a>Layers

A crate depends only on crates in lower layers. Crates in the same layer never depend on each other.

| Layer | Crates | Role |
| --- | --- | --- |
| 0 | `artic-core`, `artic-macros`, `artic-xaml` | Foundations with no artic dependencies: the reactive core and tree, the proc macros, and the XAML parser |
| 1 | `artic-registry`, `artic-layout`, `artic-i18n` | Type metadata, geometry and layout, localization |
| 2 | `artic-render`, `artic-style`, `artic-xaml-compiler`, `artic-xaml-runtime` | Rendering, styling and the two XAML back ends |
| 3 | `artic-controls`, `artic-platform`, `artic-cli`, `artic-lsp` | Controls, platform hosts and tooling |
| 4 | `artic-theme`, `artic-test` | The default theme and test utilities |
| 5 | `artic` | The facade that apps depend on |

## <a id="arch-dependency-graph"></a>Dependency graph

Arrows point from a crate to its dependency.

```mermaid
flowchart TD
  artic --> theme[artic-theme]
  artic --> controls[artic-controls]
  artic --> platform[artic-platform]
  artic -. hot-reload / runtime-xaml .-> runtime[artic-xaml-runtime]

  theme --> controls
  theme --> style[artic-style]

  controls --> style
  controls --> render[artic-render]
  controls --> layout[artic-layout]
  controls --> registry[artic-registry]
  controls --> macros[artic-macros]

  platform --> render
  platform --> layout

  test[artic-test] --> platform

  cli[artic-cli] --> compiler[artic-xaml-compiler]
  lsp[artic-lsp] --> compiler

  render --> layout
  style --> registry
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
| `artic-macros` | none (proc-macro; generated code names `artic-core` and `artic-registry` paths) |
| `artic-xaml` | none |
| `artic-registry` | `artic-core` |
| `artic-layout` | `artic-core` |
| `artic-i18n` | `artic-core` |
| `artic-render` | `artic-core`, `artic-layout` |
| `artic-style` | `artic-core`, `artic-registry` |
| `artic-xaml-compiler` | `artic-xaml`, `artic-registry` |
| `artic-xaml-runtime` | `artic-xaml`, `artic-registry`, `artic-core` |
| `artic-controls` | `artic-core`, `artic-registry`, `artic-layout`, `artic-render`, `artic-style`, `artic-macros` |
| `artic-platform` | `artic-core`, `artic-layout`, `artic-render` |
| `artic-cli` | `artic-xaml`, `artic-xaml-compiler` |
| `artic-lsp` | `artic-xaml`, `artic-xaml-compiler` |
| `artic-theme` | `artic-controls`, `artic-style` |
| `artic-test` | `artic-core`, `artic-layout`, `artic-render`, `artic-platform` |
| `artic` | every app-facing crate; `artic-xaml-runtime` only with the `hot-reload` or `runtime-xaml` feature |

## <a id="arch-rules"></a>Rules

- `artic-controls` does not depend on `artic-platform`. Input event types live in `artic-core`, and the facade connects the platform host to the tree. Controls stay testable without a window and portable to future targets.
- `artic-xaml` does not depend on the registry. Type references stay unresolved in the infoset, so parser tests need no registry.
- Release builds link `artic-xaml-runtime` only when the app enables `runtime-xaml`. Compiled views never touch the loader.
- App-facing crates stay free of native-only dependencies, so a web target can be added later without app changes (D24 in the spec).

## <a id="arch-why-this-differs-from-the-original-2-diagram"></a>Why this differs from the original §2 diagram

The first version of the spec drew `artic-core -> artic-layout -> artic-controls -> artic-style/platform/render`. Read with the stated arrow direction, that makes the core depend on layout and layout depend on controls, which contradicts "each layer depends only on the ones below it". The graph above is the corrected version, and the spec now points here.

## <a id="arch-checking-the-graph"></a>Checking the graph

```sh
cargo tree -e normal --workspace --depth 1
```

Every crate's direct dependencies must match the table above. A pull request that adds an edge updates this document in the same change.
