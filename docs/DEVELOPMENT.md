# prismo Development

## Local Workflow

Run the prototype:

```bash
cargo run -q
```

Build and test:

```bash
cargo test
```

Format the workspace:

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

## Add a New Plugin

Start by deciding which boundary you need:
- `crates/plugin-protocol` for the wire contract
- `crates/plugin-host` for process supervision and normalization
- `crates/plugin-sdk-rust` for a Rust authoring helper
- `plugins/example-rust` for a reference subprocess plugin

For a new Rust plugin:
- ship a plugin manifest with a command entrypoint
- read `Init` from `stdin`
- emit `Hello`, `DeclareChannels`, `SampleBatch`, and optional `Health`
- send descriptors before samples that reference them

Project-local plugin startup now flows through `prismo.toml`.

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
- separate transport ingestion from decode logic
- introduce a decoder plugin trait
- add Python and C++ SDKs on top of the shared protobuf protocol

## Notes For Future Multi-Language Plugins

The runtime is already subprocess-based and protocol-first.

That means Python and C++ support can be added by:
- keeping the same protobuf message contract
- providing language-specific SDKs
- reusing the same project-local manifest and `prismo.toml` configuration model
