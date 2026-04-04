# prismo Architecture

## Overview

`prismo` is structured as a small Rust workspace with a clear split between:
- telemetry ingestion and modeling
- plugin protocol and process hosting
- TUI rendering and interaction
- application runtime wiring
- plugin SDKs and implementations

The current architecture is intentionally simple but now uses a protocol-first plugin boundary.

## Crates

## `crates/core`

This crate owns the core telemetry concepts:
- `ChannelDescriptor`
- `ChannelValue`
- `ChannelSample`
- `TelemetryUpdate`
- `RuntimeEvent`
- `PluginHealth`
- `PluginRuntimeState`
- `TelemetryStore`

It owns the internal telemetry contract used after plugin messages are normalized by the host.

## `crates/plugin-protocol`

This crate owns the external plugin contract:
- protobuf message types
- length-prefixed frame helpers
- plugin manifest parsing
- project-local runtime config parsing

## `crates/plugin-host`

This crate owns:
- subprocess spawn and supervision
- `stdin` / `stdout` / `stderr` transport
- handshake validation
- restart policy handling
- conversion from wire messages into `RuntimeEvent`

## `crates/plugin-sdk-rust`

This crate is the first plugin authoring SDK for the subprocess protocol.

## `crates/plugin-sdk-ffi`

This crate exposes a small Diplomat-backed FFI surface on top of the Rust SDK core.
It exists so C++ plugins can reuse the same stdio framing and protobuf implementation
without reimplementing the protocol natively yet.

## `plugins/example-rust`

This crate contains the current Rust example plugin implementation.
It now runs as a subprocess plugin over the shared wire protocol.

## `plugins/example-cpp`

This target contains the current C++ example plugin implementation.
It links against the generated Diplomat C++ bindings and the Rust-backed FFI SDK.

## `crates/tui`

This crate owns:
- pane layout
- key and mouse handling
- focused text cursor behavior
- details and latest-value rendering
- channel tree expansion/collapse state
- scroll offsets and scrollbar rendering
- command/filter prompt rendering
- status bar and help overlay

It renders from `StoreSnapshot` only. It does not talk directly to plugins.

## `apps/prismo`

This crate is the executable:
- loads project-local config from `prismo.toml`
- starts plugin subprocesses through `plugin-host`
- receives `RuntimeEvent` values through a channel
- applies normalized telemetry to `TelemetryStore`
- drives the TUI render loop
- handles clipboard yank via OSC 52

## Data Flow

The current runtime looks like this:

```text
plugin subprocess -> stdio + protobuf -> plugin-host -> RuntimeEvent -> app -> TelemetryStore -> StoreSnapshot -> tui
```

Detailed flow:
1. The app reads `prismo.toml`.
2. The host resolves each configured plugin manifest and spawns the plugin subprocess.
3. The host sends `Init` on `stdin` and validates the child's `Hello`.
4. The host normalizes plugin messages into `RuntimeEvent` values.
5. The main loop drains pending events with `try_recv`.
6. The store applies descriptors, samples, plugin health, and runtime state.
7. The TUI renders from the latest snapshot.

## Telemetry Model

The current canonical value types are:
- `Bool`
- `Integer`
- `Float`
- `Text`
- `Bytes`

The store keeps:
- latest sample per channel
- recent numeric history for charting
- per-channel update counts
- the most recent observed inter-sample interval and derived rate
- plugin health snapshots
- total dropped updates caused by missing channel descriptors

## Store Behavior

Important current store rules:
- channels are keyed by `(plugin_id, path)` in a `BTreeMap`
- numeric history retention is capped at 64 samples
- staleness is adaptive: a channel is stale when time since last sample exceeds `3x` the most recent observed interval
- channels with only one sample fall back to a `3s` initial stale threshold
- `total_updates` counts applied update batches, not individual samples
- `dropped_updates` increments if a sample arrives before its descriptor exists

## Plugin Model

Today the plugin model is subprocess-first:
- each plugin runs as a child process
- the host sends `Init` on `stdin`
- the plugin writes framed protobuf messages on `stdout`
- free-form logs go to `stderr`

The current Rust and C++ example plugins send:
- `Hello` on startup
- `DeclareChannels` once
- randomized `SampleBatch` messages on each interval tick
- `Health` containing `emitted_updates`

## Build System

Bazel is the primary build system:
- `MODULE.bazel` pins the Bazel module graph
- `rules_rust` provides the hermetic Rust toolchain
- `toolchains_llvm` provides the hermetic C++ toolchain used for the in-repo C++ plugin
- `crate_universe` reads the Cargo workspace metadata and resolves external Rust crates for Bazel
- each workspace package has its own `BUILD.bazel`

The current C++ path is intentionally Rust-backed:
- `crates/plugin-sdk-ffi` builds a Rust static library
- Bazel runs a Rust Diplomat codegen tool and exports the generated C and C++ headers
- `plugins/example-cpp` links that Rust library into a Bazel-built C++ binary

Cargo manifests remain in the repo as dependency metadata and editor/tooling support, not as the primary execution path.

## UI Structure

The UI has three focusable regions:
- `Details`
- `Latest Value`
- `Channels`

The overall layout is:
- wide terminals: details on the left, channels on the right, footer on the bottom
- narrow terminals: details on top, channels below, footer on the bottom

The current channels pane is a tree:
- namespaces are derived from dot-separated channel paths
- namespaces can be collapsed individually with `Enter`
- the full tree can be toggled with `z`
- selecting a namespace shows namespace summary details plus descendant channels in the left pane

Renderers:
- numeric values: line chart plus textual summary
- text/integer/bool values: simple text block
- bytes values: hex and ASCII summary

The details pane is intentionally fixed-height and non-scrolling:
- channel details render as a five-line summary block
- namespace details use the same aligned two-column structure
- latest-value content remains the scrollable/expanded pane for long payloads

## Input Model

The input model is split by focus:
- `Channels`: `j/k`, mouse clicks, and mouse wheel choose the active row
- `Details`: focusable and copyable line-by-line, but intentionally non-scrolling
- `Latest Value`: cursor movement and mouse wheel act inside the visible text summary

Copy behavior:
- in `Channels`, `y` copies the current live channel value
- in `Details` or `Latest Value`, `y` copies the current line under the text cursor

Command/filter behavior:
- `/` opens a persistent filter prompt; a non-empty filter remains visible until cleared
- `Esc` while editing the filter clears it entirely
- `:` opens command mode
- `:q` is the current quit command

## Status and Help

The status bar is intentionally compact:
- left side: `:q quit`, `: command`, `? help`, current focus, and transient notices
- right side: store totals and plugin health

The help overlay is the canonical shortcut reference for the app.

## Design Limits of the Current Prototype

The current implementation does not yet have:
- transport plugins separate from decoders
- multiple instances of the same plugin type
- persistence or replay
- robust clipboard fallback behavior for terminals without OSC 52 support
- responsive/truncated formatting for very large structured payloads beyond the current text renderers
