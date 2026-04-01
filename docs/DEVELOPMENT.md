# prismo Development

## Local Workflow

Run the prototype:

```bash
cargo run -p prismo
```

Format and check:

```bash
cargo fmt
cargo check
```

## Current Interaction Model

Use `?` inside the app for the live help overlay.

Important behavior to remember while developing:
- `Tab` changes which pane has focus
- `Channels` focus changes which channel is selected
- `Details` and `Latest Value` focus move a text cursor within those panes
- `y` behaves differently by focus
- `/` opens the filter prompt and temporarily takes over keyboard input

## Clipboard Behavior

Copy uses OSC 52 terminal clipboard output from the app crate.

That means:
- it works best in terminals that explicitly support OSC 52
- it may be ignored in some local terminals, remote shells, or multiplexers depending on configuration
- the UI still shows success or failure messages based on whether the write itself succeeded

## Where To Extend The Prototype

## Add a New Source Plugin

Start in `crates/telemetry-core`:
- implement `SourcePlugin`
- emit `TelemetryUpdate` batches
- send descriptors before samples that reference them
- include `PluginHealth` if you want footer statistics

Right now the app directly instantiates `SyntheticPlugin` in `crates/telemetry-app/src/main.rs`. Replacing that with source selection is the next logical step.

## Add New Renderers

Add rendering logic in `crates/telemetry-tui/src/lib.rs`.

The current renderer split is by `ChannelValue`:
- bytes -> hex/ASCII block
- numeric -> text summary plus line chart
- text/integer/bool -> text block

If you add richer value kinds later, this is where new pane renderers should go.

## Evolve The Data Model

If you need more telemetry semantics, start in `crates/telemetry-core/src/model.rs`.

Likely future expansions:
- enums with labels
- structured key/value values
- units and formatting hints
- source stream IDs
- richer quality/validity metadata

## Suggested Next Engineering Steps

- add unit tests for `TelemetryStore`
- snapshot-test the TUI rendering surface
- move the hard-coded plugin construction behind a config or CLI layer
- separate transport ingestion from decode logic
- introduce a decoder plugin trait
- define an external plugin protocol for Python and C++ integrations

## Notes For Future Multi-Language Plugins

The current code is Rust-first and in-process. That is fine for this prototype.

If the project later needs Python or C++ plugins, a process boundary is likely the cleanest next step:
- keep `TelemetryUpdate` as the conceptual contract
- define a transport format for descriptors, samples, and health
- run third-party decoders as sidecars instead of Rust dynamic libraries

That path preserves the current workspace shape without locking the project into a Rust-only extension model.
