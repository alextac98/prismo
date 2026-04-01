# prismo Architecture

## Overview

`prismo` is structured as a small Rust workspace with a clear split between:
- telemetry ingestion and modeling
- TUI rendering and interaction
- application runtime wiring

The current architecture is intentionally simple but already shaped around a plugin boundary.

## Crates

## `crates/telemetry-core`

This crate owns the core telemetry concepts:
- `ChannelDescriptor`
- `ChannelValue`
- `ChannelSample`
- `TelemetryUpdate`
- `PluginHealth`
- `TelemetryStore`
- `SourcePlugin`

It also contains the current synthetic plugin implementation.

## `crates/telemetry-tui`

This crate owns:
- pane layout
- key and mouse handling
- focused text cursor behavior
- detail and latest-value rendering
- status bar and help overlay

It renders from `StoreSnapshot` only. It does not talk directly to plugins.

## `crates/telemetry-app`

This crate is the executable:
- creates the Tokio runtime
- spawns the source plugin
- receives `TelemetryUpdate` batches through a channel
- applies them to `TelemetryStore`
- drives the TUI render loop
- handles clipboard yank via OSC 52

## Data Flow

The current runtime looks like this:

```text
SyntheticPlugin -> mpsc::Sender<TelemetryUpdate> -> telemetry-app -> TelemetryStore -> StoreSnapshot -> telemetry-tui
```

Detailed flow:
1. The app creates a bounded Tokio MPSC channel.
2. The synthetic plugin is spawned and periodically sends `TelemetryUpdate`.
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
- plugin health snapshots
- total dropped updates caused by missing channel descriptors

## Store Behavior

Important current store rules:
- channels are keyed by path in a `BTreeMap`
- numeric history retention is capped at 64 samples
- a channel is considered stale after 3 seconds without updates
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

This is a minimal in-process plugin seam, not yet a full multi-language plugin architecture.

## UI Structure

The UI has three focusable regions:
- `Details`
- `Latest Value`
- `Channels`

The overall layout is:
- wide terminals: details on the left, channels on the right, footer on the bottom
- narrow terminals: details on top, channels below, footer on the bottom

Renderers:
- numeric values: line chart plus textual summary
- text/integer/bool values: simple text block
- bytes values: hex and ASCII summary

## Input Model

The input model is split by focus:
- `Channels`: `j/k` and mouse choose the active channel
- `Details` and `Latest Value`: cursor movement acts inside the text shown in that pane

Copy behavior:
- in `Channels`, `y` copies the current live channel value
- in `Details` or `Latest Value`, `y` copies the current line under the text cursor

## Status and Help

The status bar is intentionally compact:
- left side: `q quit`, `? help`, current focus, and transient notices
- right side: store totals and plugin health

The help overlay is the canonical shortcut reference for the app.

## Design Limits of the Current Prototype

The current implementation does not yet have:
- transport plugins separate from decoders
- config files or CLI source selection
- persistence or replay
- tests
- external process plugins
- robust clipboard fallback behavior for terminals without OSC 52 support
