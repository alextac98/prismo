# Prismo - A TUI Telemetry Viewer

![prismo screenshot](./docs/AppScreenshot.png)

`prismo` is a terminal telemetry viewer prototype written in Rust for embedded and target-side debugging.

The current prototype is intentionally small:
- a Rust workspace with a TUI app, an internal telemetry core, a protobuf-based plugin protocol, and example Rust and C++ plugins
- a two-pane telemetry UI built with `ratatui` and `crossterm`
- subprocess example plugins that generate randomized or synthetic telemetry so the UI can be developed without a live target
- a workspace split that keeps app wiring, UI, and telemetry contracts separate

## Current State

The app already supports:
- a nested channel tree with collapsible namespaces
- a channel list with live/stale markers
- a fixed summary-style details pane for the selected item
- numeric history as a line chart
- bytes and text renderers
- vim-like navigation
- mouse selection and mouse-wheel scrolling in scrollable panes
- scrollbars for scrollable panes
- persistent `/` filter UI
- a minimal `:` command mode with `:q`
- copy/yank support using OSC 52 terminal clipboard sequences
- a help overlay
- a minimum supported window size with a fallback message

The current examples are the Rust and C++ plugins, and both run through the same subprocess protocol path.

## Workspace Layout

- `app`: the `prismo` binary and runtime loop
- `crates/core`: internal telemetry data model, store, and runtime snapshots
- `crates/plugin-protocol`: protobuf messages, framing, manifests, and plugin discovery
- `crates/plugin-host`: subprocess supervision and wire-message normalization
- `crates/plugin-sdk/cpp`: Diplomat-backed C++ SDK surface
- `crates/plugin-sdk/rust`: Rust plugin authoring helper
- `plugins/example-cpp`: C++ example plugin built through Bazel and the FFI SDK
- `crates/tui`: layout, rendering, input handling, and help UI
- `plugins/example-rust`: Rust example plugin implementation
- `docs/`: project documentation

## Build and Run

Requirements:
- a terminal with alternate-screen support
- OSC 52 clipboard support if you want yank/copy to reach your terminal clipboard

Current in-repo development path:

```bash
bazel run //app:example_prismo
```

That target builds a runnable bundle containing:
- the `prismo` app
- the Rust example plugin
- the C++ example plugin

At runtime, `prismo` discovers plugins from:
- `./plugins` relative to the `prismo` executable
- or an explicit `--plugins /path/to/plugins` override

Build and test:

```bash
cargo test
bazel build //app:example_prismo
```

Bazel consumers can load [defs.bzl](/Users/alex/code/alextac98/prismo/bazel/defs.bzl) and use:
- `prismo_plugin`: package an executable plus a checked-in `prismo-plugin.toml`
- `prismo_bundle`: assemble `prismo` plus plugins into the runtime bundle layout; the bundle target itself is runnable with `bazel run`

Format:

```bash
cargo fmt
```

Cargo manifests are still present because `rules_rust` `crate_universe` reads the workspace dependency graph from [Cargo.toml](/Users/alex/code/alextac98/prismo/Cargo.toml) and [Cargo.lock](/Users/alex/code/alextac98/prismo/Cargo.lock).

## Controls

The app shows a help overlay with `?`.

The main shortcuts today are:
- `:q`: quit
- `:`: open command mode
- `?`: open or close help
- `Tab`: cycle focus between `Details`, `Latest Value`, and `Channels`
- `j/k`: move channel selection in `Channels`, or move the text cursor vertically in focused text panes
- `h/l` or arrow keys: move the text cursor in focused text panes
- `g/G`: jump to the first or last channel
- `Enter`: collapse or expand the selected namespace in `Channels`
- `z`: toggle the whole channel tree collapsed or expanded
- `/`: open the channel filter
- `y`: copy the current line in `Details` or `Latest Value`, or copy the live channel value from `Channels`
- `Esc`: cancel filter input, command input, or help
- mouse left click: focus/select within a pane
- mouse wheel: scroll the hovered scrollable pane

## Status Bar

The status bar is split into two parts:
- left: quit/help hints, focus, and transient notices such as copy success or failure
- right: app-wide counters plus plugin state and health

Example:

```text
:q quit  : command  ? help  focus:channels                      total:42 dropped:0  example-rust:running r0 u:42 d:0
```

`total` is the number of update batches applied by the store. `r0` is the restart count for the plugin instance.

## Plugin Model Today

The current plugin boundary is protocol-first:
- plugins are child processes spawned by `prismo`
- `stdin` / `stdout` carry length-prefixed protobuf frames
- the host normalizes those frames into internal telemetry updates
- the TUI renders store snapshots
- plugin manifests are checked into plugin directories and packaged unchanged
- plugins are discovered from a sibling `plugins/` directory or an explicit `--plugins` override

Reference implementations live in `plugins/example-rust` and `plugins/example-cpp`. They emit channel descriptors on startup and then periodically emit samples and plugin health over the shared protocol.

## Notes About Freshness

`prismo` marks a channel stale using the most recent observed sample interval:
- if the latest gap exceeds `3x` the last interval, the channel is stale
- if there is only one sample so far, the app falls back to an initial `3s` threshold

## Next Likely Steps

- split transport and decode stages more explicitly
- add a Python SDK on top of the shared protocol
- add tests for rendering, store behavior, and plugin contracts

## Docs

- [docs/README.md](/Users/alex/code/alextac98/prismo/docs/README.md)
- [docs/ARCHITECTURE.md](/Users/alex/code/alextac98/prismo/docs/ARCHITECTURE.md)
- [docs/DEVELOPMENT.md](/Users/alex/code/alextac98/prismo/docs/DEVELOPMENT.md)
