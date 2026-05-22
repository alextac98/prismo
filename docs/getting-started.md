---
id: getting-started
title: Getting Started
slug: /getting-started
description: Run Prismo locally and get familiar with the core workflow.
---

# Getting Started

## Requirements

- a terminal with alternate-screen support
- OSC 52 clipboard support if you want `y` to reach your terminal clipboard.
- Either Bazel or Cargo for building and running locally (currently only Bazel is supported for C++ plugins)

## Run Prismo

The fastest way to try Prismo locally is through Bazel.

With Bazel, you can try the example bundle that includes both C++ and Rust plugins:

```bash
bazel run //:prismo
```

## Using Prismo

Prismo is designed for quick inspection of live data:

- the channel tree helps you move through available telemetry
- the details pane summarizes the selected item
- the latest-value pane shows the current payload in more detail
- filtering and keyboard navigation are meant to keep the workflow fast

### Controls

The in-app help overlay can be opened with `?` key. It is kept up-to-date with the latest features.

Users can navigate with either the keyboard or mouse:

- `tab` cycles focus between the panes
- `j`/`k`/`h`/`l` or arrow keys move the cursor around in focused panes
- `g`/`G` jump to the first or last channel in the tree
- `Enter` collapses or expands namespaces in the channel tree
- `z` toggles the whole channel tree collapsed or expanded

There are also several actions that users can use:

- `y` copies the data that the cursor is on
- `/` opens the channel filter and allows for simple filtering of names
- `:` opens the command mode (which currently only supports `:q` to quit, but is designed for future extensibility)

## Build and Test

Use Bazel as the main repository build and test surface:

```bash
bazel build //apps/prismo
bazel test //apps/prismo:cpp_smoke_test
```

The Rust workspace tests run through Cargo:

```bash
cargo test
```
