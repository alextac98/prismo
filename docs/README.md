# prismo Docs

This directory documents the current `prismo` prototype.

## Documents

- [ARCHITECTURE.md](/Users/alex/code/alextac98/prismo/docs/ARCHITECTURE.md): workspace structure, runtime flow, store behavior, and UI structure
- [DEVELOPMENT.md](/Users/alex/code/alextac98/prismo/docs/DEVELOPMENT.md): local development workflow, controls, and implementation notes for extending the prototype

## Scope

These docs describe what exists in the repository now:
- a Bazel-built Rust TUI prototype
- Rust and C++ example plugins running through the same subprocess protocol
- a Rust-backed Diplomat FFI bridge for C++ plugin authors
- a store-backed viewer with a collapsible channel tree
- fixed details plus scrollable latest-value content
- vim-style filter and command prompts

They still do not describe a production deployment flow or a complete out-of-repo plugin packaging story yet.
