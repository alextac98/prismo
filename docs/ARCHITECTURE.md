# prismo Architecture

## Overview

`prismo` is structured as a small Rust workspace with a clear split between:
- telemetry ingestion and modeling
- TUI rendering and interaction
- application runtime wiring
- built-in plugin implementations

The current architecture is intentionally simple but already shaped around a plugin boundary.

## Crates

## `crates/core`

This crate owns the core telemetry concepts:
- `ChannelDescriptor`
- `ChannelValue`
- `ChannelSample`
- `TelemetryUpdate`
- `PluginHealth`
- `TelemetryStore`
- `SourcePlugin`
- `PluginHandle`

It owns both the telemetry contract and the current in-process Rust source-plugin boundary.

## `plugins/synthetic`

This crate contains the current synthetic source implementation.

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
- selects and spawns a source implementation
- creates the Tokio runtime
- receives `TelemetryUpdate` batches through a channel
- applies them to `TelemetryStore`
- drives the TUI render loop
- handles clipboard yank via OSC 52

## Data Flow

The current runtime looks like this:

```text
SyntheticPlugin -> SourcePlugin -> mpsc::Sender<TelemetryUpdate> -> app -> TelemetryStore -> StoreSnapshot -> tui
```

Detailed flow:
1. The app creates a bounded Tokio MPSC channel.
2. The app constructs the synthetic source and spawns it through the `core` `SourcePlugin` trait.
3. The main loop drains pending updates with `try_recv`.
4. The store applies descriptors, samples, and plugin health.
5. The TUI renders from the latest snapshot.

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
- channels are keyed by path in a `BTreeMap`
- numeric history retention is capped at 64 samples
- staleness is adaptive: a channel is stale when time since last sample exceeds `3x` the most recent observed interval
- channels with only one sample fall back to a `3s` initial stale threshold
- `total_updates` counts applied update batches, not individual samples
- `dropped_updates` increments if a sample arrives before its descriptor exists

## Plugin Model

Today the plugin model is source-only:
- a `SourcePlugin` identifies itself with `id()`
- it is responsible for spawning its own async task
- it emits `TelemetryUpdate` batches

The synthetic plugin sends:
- descriptors on the first update
- randomized sample values on each interval tick
- plugin health containing `emitted_updates`

This is a minimal Rust runtime seam, not yet a full multi-language plugin architecture.

## Build System

Bazel is the primary build system:
- `MODULE.bazel` pins the Bazel module graph
- `rules_rust` provides the hermetic Rust toolchain
- `crate_universe` reads the Cargo workspace metadata and resolves external Rust crates for Bazel
- each workspace package has its own `BUILD.bazel`

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
- config files or CLI source selection
- persistence or replay
- external process plugins
- robust clipboard fallback behavior for terminals without OSC 52 support
- responsive/truncated formatting for very large structured payloads beyond the current text renderers
