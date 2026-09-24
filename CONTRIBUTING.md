# Contributing to artic

## Setup

1. Install [rustup](https://rustup.rs).
2. Clone the repository. The toolchain version is pinned in `rust-toolchain.toml`, and `rustup` installs it with `rustfmt` and `clippy` the first time you run `cargo` in the repo.
3. Install the extra tools used below:

   ```sh
   cargo install cargo-deny cargo-insta --locked
   ```

The crate layout and the rules for dependencies between crates are in [docs/architecture.md](docs/architecture.md). The design is in [docs/spec/technical_spec.md](docs/spec/technical_spec.md).

## Checks

CI runs these on every pull request. Run them locally before pushing:

| Check | Command | CI job |
| --- | --- | --- |
| Build | `cargo build --workspace --all-targets` | Test (Windows, macOS, Ubuntu) |
| Tests | `cargo test --workspace` | Test (Windows, macOS, Ubuntu) |
| Formatting | `cargo fmt --all --check` | Lint |
| Lints | `cargo clippy --workspace --all-targets -- -D warnings` | Lint |
| Docs | `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS=-D warnings` | Docs |
| Dependency policy | `cargo deny check` | Deny |

On Windows PowerShell, set the docs flag with `$env:RUSTDOCFLAGS="-D warnings"` before running `cargo doc`.

The required status checks for `main` are `Test (windows-latest)`, `Test (macos-latest)`, `Test (ubuntu-latest)`, `Lint`, `Docs` and `Deny`.

## Snapshot tests

Parser and compiler tests use [insta](https://insta.rs) snapshots. When a change alters a snapshot, the test fails and writes a `.snap.new` file next to it. Review and accept the changes with:

```sh
cargo insta review
```

Commit the updated `.snap` files with the change that caused them. Never commit `.snap.new` files.

## Dependencies

`deny.toml` defines the dependency policy. Dependencies must use one of the allowed licenses, come from crates.io, and have no open security advisories. A new dependency that fails `cargo deny check` needs a discussion in its pull request before the policy changes.

A new dependency between workspace crates must follow the layers in [docs/architecture.md](docs/architecture.md). Update that document in the same pull request.

## Issues and labels

Every issue has three kinds of labels:

| Prefix | Meaning | Example |
| --- | --- | --- |
| `type:` | Kind of work: `feature`, `test`, `infra`, `docs`, `spike`, `design` | `type:feature` |
| `crate:` | Crates the work touches; `crate:workspace` for repo-wide changes | `crate:artic-core` |
| `platform:` | Only when the work is specific to a platform | `platform:android` |

Issues are grouped into milestones. The "Depends on" line in an issue lists what has to land first.

## Pull requests

- Keep one issue per pull request, and put `Closes #<number>` in the description.
- Keep unrelated changes out, including drive-by formatting.
- Add or update tests for the behavior you change.
- Update the docs and the spec when the change affects them.
- Make sure every check above passes.

## Commit messages

- Subject line in the imperative mood ("Add", "Fix", "Remove"), at most 72 characters, no trailing period.
- A blank line, then a body that explains what changed and why, wrapped at 72 characters. Skip the body for trivial changes.
- Reference issues in the pull request description, not in the subject line.

```
Add Grid star sizing

Distribute the remaining space across star rows and columns in
proportion to their weights, after Auto and fixed sizes are measured.
```

## License

By contributing, you agree that your contributions are dual licensed under the MIT and Apache 2.0 licenses, as described in the [README](README.md#license).
