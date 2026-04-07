---
id: intro
title: Introduction
slug: /
description: Overview of the current Prismo prototype and its scope.
---

This directory documents the current `prismo` prototype.

These files are developer-facing repository docs. They are meant to track the code and build shape in this repo, not serve as polished user-facing product docs.

## Documents

- [ARCHITECTURE.md](/Users/alex/code/alextac98/prismo/docs/ARCHITECTURE.md): workspace structure, runtime flow, store behavior, and UI structure
- [DEVELOPMENT.md](/Users/alex/code/alextac98/prismo/docs/DEVELOPMENT.md): local development workflow, controls, and implementation notes for extending the prototype

## Scope

These docs describe what exists in the repository now:

- a Bazel-built Rust workspace centered on a TUI app
- a protocol-first plugin boundary shared by Rust and C++ examples
- a Rust-backed Diplomat FFI bridge for C++ plugin authors
- bundle-first plugin discovery relative to the `prismo` executable
- checked-in plugin manifests that Bazel packages unchanged
- a store-backed viewer with a collapsible channel tree
- fixed details plus scrollable latest-value content
- vim-style filter and command prompts

## Read This Next

- [Getting Started](./GETTING_STARTED.md) for the current local workflow
- [Architecture](./ARCHITECTURE.md) for workspace structure and runtime flow
- [Development](./DEVELOPMENT.md) for extension points and implementation notes

## Current Scope

The prototype already supports:

- nested channel trees with collapsible namespaces
- live and stale state markers
- numeric history charting
- text and bytes renderers
- vim-like navigation and filter prompts
- mouse interaction in scrollable panes
- OSC 52 clipboard copy support

The docs still do not describe a production deployment flow or a complete
out-of-repo plugin packaging story yet.
