---
id: getting-started
title: Getting Started
slug: /getting-started
description: Build, run, and explore the current Prismo prototype.
---

# Getting Started

## Requirements

- a terminal with alternate-screen support
- OSC 52 clipboard support if you want `y` to reach your terminal clipboard
- Rust tooling for the current `cargo run` workflow
- Bazel for hermetic builds and tests

## Current Local Run Path

The current easiest way to run the prototype locally is still through Cargo:

```bash
cargo run -q -- --plugins ./plugins/example-rust
```

Run the C++ example plugin with:

```bash
cargo run -q -- --plugins ./plugins/example-cpp
```

## Build and Test

Use Bazel as the main build and test surface:

```bash
bazel build //apps/prismo
bazel test //apps/prismo:cpp_smoke_test
```

The Rust workspace tests currently run through Cargo:

```bash
cargo test
```

## Important Controls

Open the in-app help overlay with `?`.

Common shortcuts:

- `:q` to quit
- `Tab` to cycle focus
- `j` / `k` to move in lists and text
- `Enter` to collapse or expand namespaces
- `/` to open the channel filter
- `y` to copy the focused value or line

## Where To Go Next

- [Introduction](./README.md)
- [Architecture](./ARCHITECTURE.md)
- [Development](./DEVELOPMENT.md)
