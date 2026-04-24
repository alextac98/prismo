---
id: development
title: Development
slug: /development
description: Local development workflow, extension points, and contributor notes.
---

# Development

The primary build system used by Prismo is Bazel, which provides a hermetic and reproducible build environment for both the Rust and C++ parts of the project. Cargo manifests are still present for the Rust crates to allow for IDE integration and because `rules_rust` uses them to understand the workspace dependency graph.

## Versioning

This project strictly follows [semantic versioning](https://semver.org/), summarized below:

- MAJOR version when you make incompatible API changes
- MINOR version when you add functionality in a backwards compatible manner
- PATCH version when you make backwards compatible bug fixes

Note: read the whole spec at https://semver.org/ for more details, especially about pre-release versions and build metadata.

Version updates are anchored on `MODULE.bazel`, which acts as the single source of truth for the repo version. Cargo package versions are intentionally fixed at `0.0.0` because the Rust crates are maintained for in-repo development and IDE support rather than publication.

When making a change that requires a version bump, update `MODULE.bazel`, then run the tests:

```bash
$EDITOR MODULE.bazel
bazel test //...
```

After that change lands on `main`, GitHub Actions automatically creates a GitHub release tagged `v<version>` with generated release notes. The release job only runs when the `MODULE.bazel` version actually changed on that push.

When a new GitHub release is created, CI also publishes the Bazel module to the Bazel Central Registry by opening a PR against `bazelbuild/bazel-central-registry`. The workflow expects a fork at `alextac98/bazel-central-registry` and a repository or organization GitHub Actions secret named `BCR_PUBLISH_TOKEN` that can push to that fork and open the upstream PR. 

## Local Workflow

The main in-repo development path is the Bazel example bundle:

```bash
bazel run //app:example_prismo
# Or the shortcut
bazel run //:prismo
```

That target bundles the app with both in-repo example plugins so you can work on the full flow locally.

Bazel will automatically manage any tests that are defined. You can run all tests with:

```bash
bazel test //...
```

Format the workspace:

```bash
bazel run //:format
```

## Clipboard Behavior

Copy uses OSC 52 terminal clipboard output from the app crate.

That means:
- it works best in terminals that explicitly support OSC 52
- it may be ignored in some local terminals, remote shells, or multiplexers depending on configuration
- the UI still shows success or failure messages based on whether the write itself succeeded

A shortlist of known good OSC 52 terminals include:
- [xterm](https://invisible-island.net/xterm/)
- [Ghostty](https://github.com/ghostty/ghostty)
- [iTerm2](https://iterm2.com/)
- [Kitty](https://sw.kovidgoyal.net/kitty/)
- [Alacritty](https://github.com/alacritty/alacritty)
- [Windows Terminal](https://aka.ms/terminal)

Notably, Apple Terminal does not support OSC 52.

## Add a New Plugin

Plugins are designed to exist anywhere, either in this repo or closed sourced in your own projects. The only requirement is that they follow the protobuf-over-stdio contract and provide a manifest for discovery.

If you would like to build a plugin along with the project, please place it in the `plugins/` directory and add a `prismo-plugin.toml` manifest. The example Rust and C++ plugins can be used as a reference.

For a new Rust plugin:
- ship a plugin manifest with a command entrypoint
- read `Init` from `stdin`
- emit `Hello`, `DeclareChannels`, `SampleBatch`, and optional `Health`
- send descriptors before samples that reference them

The normal runtime path is discovery-first:
- bundle plugins under `./plugins` relative to the `prismo` executable
- use `--plugins /path/to/plugins` when you want to point `prismo` at a different plugin directory
- for local repo development, use the Bazel example bundle targets so the plugin binaries are laid out next to their manifests
- keep `prismo-plugin.toml` checked into the plugin directory rather than generating it inside the build

For the current C++ path:
- the canonical protocol is still the protobuf stdio contract
- C++ code talks to a small Diplomat-generated C++ API
- the Diplomat layer calls into the Rust SDK core
- Bazel generates the C and C++ bindings during the `cpp_sdk` build, so there is no separate manual regeneration step

For downstream Bazel integration:
- load `//bazel:defs.bzl`
- use `prismo_plugin` to package a plugin executable plus a checked-in manifest
- use `prismo_bundle` to assemble `prismo` plus packaged plugins; the bundle target itself is runnable with `bazel run`

## Renderers

The TUI rendering logic is intentionally separate from the core data model and plugin protocol, so new renderers can be added without touching the runtime or plugin contract. Rendering logic lives in `crates/tui/src/lib.rs`.

The current renderer split is by `ChannelValue`:
- bytes -> hex/ASCII block
- numeric -> text summary plus line chart
- text/integer/bool -> text block

A few UI rules matter when extending renderers:
- `Details` should stay compact and fixed-height
- long payloads belong in `Latest Value`
- scrollable panes should use the shared scrollbar path
- visible text colors now carry semantics similar to the channel tree, so keep label/value/status styling consistent

If you add richer value kinds later, this is where new pane renderers should go.

## Evolve The Data Model

If you need more telemetry semantics, start in `crates/core/src/model.rs`.

Likely future expansions:
- enums with labels
- structured key/value values
- units and formatting hints
- source stream IDs
- richer quality/validity metadata

## Website and Docs

The project website is built with Docusaurus from `website/`, and it uses the
markdown files in `docs/` as the source for the `/docs` section of the site.

Build the static site with:

```bash
bazel build //website:site
```

The current prototype is intentionally small:
- a Rust workspace with a TUI app, an internal telemetry core, a protobuf-based plugin protocol, and example Rust and C++ plugins
- a two-pane telemetry UI built with `ratatui` and `crossterm`
- subprocess example plugins that generate randomized or synthetic telemetry so the UI can be developed without a live target
- a workspace split that keeps app wiring, UI, and telemetry contracts separate

## Want to Contribute?

We appreciate any contributions! If you want to contribute, please open an issue and a PR with your proposed change. We are happy to provide feedback and guidance on how to get your contribution merged. If you don't know what to get started on, take a look at open issues with the "good first issue" label.

## Notes For Future Multi-Language Plugins

The runtime is already subprocess-based and protocol-first.

That means new language support can be added by:
- keeping the same protobuf message contract
- providing language-specific SDKs
- reusing the same manifest and bundle layout model

The repo now demonstrates this in two ways:
- Rust plugins through `crates/plugin-sdk/rust`
- C++ plugins through `crates/plugin-sdk/cpp` and the generated Diplomat bindings
