# Prismo - A TUI Telemetry Viewer

![prismo screenshot](./docs/AppScreenshot.png)

`prismo` is a terminal telemetry viewer prototype written in Rust for embedded and target-side debugging.

The current prototype is intentionally small:
- a Bazel-first Rust workspace with a TUI app, core telemetry model, and a Rust example plugin
- a two-pane telemetry UI built with `ratatui` and `crossterm`
- a stub `SourcePlugin` implementation that generates randomized telemetry so the UI can be developed without a live target
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

The current source is the Rust example plugin only. Real source loading, decoder plugins, config files, and external plugin processes are not implemented yet.

## Workspace Layout

- `apps/prismo`: the `prismo` binary and runtime loop
- `crates/core`: telemetry data model, store, and Rust `SourcePlugin` contract
- `crates/tui`: layout, rendering, input handling, and help UI
- `plugins/example-rust`: Rust example plugin implementation
- `docs/`: project documentation

## Build and Run

Requirements:
- Bazel 8.x
- a terminal with alternate-screen support
- OSC 52 clipboard support if you want yank/copy to reach your terminal clipboard

Run:

```bash
bazel run //:prismo
```

Build and test:

```bash
bazel build //...
bazel test //...
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
- right: app-wide counters and plugin health

Example:

```text
:q quit  : command  ? help  focus:channels                      total:42 dropped:0  example-rust updates:42 dropped:0
```

`total` is the number of update batches applied by the store. `example-rust updates` is the update count reported by the example plugin itself.

## Plugin Model Today

The current plugin boundary is minimal:
- `core` defines `SourcePlugin`
- `SourcePlugin` produces `TelemetryUpdate` messages
- the app pushes those updates into `TelemetryStore`
- the TUI renders store snapshots

The example source in `plugins/example-rust` is the only implementation right now. It emits channel descriptors on startup and then periodically emits samples and plugin health.

## Notes About Freshness

`prismo` marks a channel stale using the most recent observed sample interval:
- if the latest gap exceeds `3x` the last interval, the channel is stale
- if there is only one sample so far, the app falls back to an initial `3s` threshold

## Next Likely Steps

- move source selection behind config or CLI arguments
- add a transport-level contract for external process plugins
- split transport and decode stages more explicitly
- add tests for rendering, store behavior, and plugin contracts
- define an external plugin protocol for non-Rust implementations

## Docs

- [docs/README.md](/Users/alex/code/alextac98/prismo/docs/README.md)
- [docs/ARCHITECTURE.md](/Users/alex/code/alextac98/prismo/docs/ARCHITECTURE.md)
- [docs/DEVELOPMENT.md](/Users/alex/code/alextac98/prismo/docs/DEVELOPMENT.md)
