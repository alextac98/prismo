# prismo Development

## Local Workflow

Run the prototype:

```bash
bazel run //:prismo
```

Build and test:

```bash
bazel build //...
bazel test //...
```

Dependency updates still flow through the Cargo workspace metadata that Bazel consumes through `crate_universe`:

```bash
cargo generate-lockfile
CARGO_BAZEL_REPIN=1 bazel sync --only=crates
```

Formatting is still simplest through:

```bash
cargo fmt
```

## Current Interaction Model

Use `?` inside the app for the live help overlay.

Important behavior to remember while developing:
- `Tab` changes which pane has focus
- `Channels` focus changes which row in the tree is selected
- `Details` is a fixed summary pane with a cursor for copy, but it does not scroll
- `Latest Value` focus moves a text cursor within that pane and may scroll
- `y` behaves differently by focus
- `/` opens the filter prompt and temporarily takes over keyboard input
- `:` opens command mode, and `:q` is the only quit path

## Clipboard Behavior

Copy uses OSC 52 terminal clipboard output from the app crate.

That means:
- it works best in terminals that explicitly support OSC 52
- it may be ignored in some local terminals, remote shells, or multiplexers depending on configuration
- the UI still shows success or failure messages based on whether the write itself succeeded

## Where To Extend The Prototype

## Add a New Source Plugin

Start by deciding which boundary you need:
- `crates/core` for shared telemetry types, store behavior, and the Rust `SourcePlugin` trait
- `plugins/example-rust` for an example in-process Rust source implementation

For a new Rust source:
- implement `SourcePlugin`
- emit `TelemetryUpdate` batches
- send descriptors before samples that reference them
- include `PluginHealth` if you want footer statistics

Right now the app directly instantiates `ExampleRustPlugin` in `apps/prismo/src/main.rs`. Replacing that with source selection is the next logical step.

## Add New Renderers

Add rendering logic in `crates/tui/src/lib.rs`.

The current renderer split is by `ChannelValue`:
- bytes -> hex/ASCII block
- numeric -> text summary plus line chart
- text/integer/bool -> text block

A few current UI rules matter when extending renderers:
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

## Suggested Next Engineering Steps

- add documentation or tests around the current tree/filter/command interaction model
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
- run third-party decoders as sidecars instead of Rust dynamic libraries or in-process foreign runtimes

That path preserves the current workspace shape without locking the project into a Rust-only extension model.
