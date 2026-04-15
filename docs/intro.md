---
id: intro
title: Introduction
slug: /
description: What Prismo is, what it is for, and how the docs are organized.
---

Prismo is a TUI-based telemetry viewer for debugging systems directly in a terminal.

It is built for engineers who want to inspect live data quickly without leaving
the shell. Prismo has minimal dependencies, can be deployed, and is designed to be
extended to whatever telemetry source you may have with a plugin system. It loosely
follows vim flow with a focus on keyboard navigation without giving up mouse support.

Typical use cases include:

- debugging Linux-based embedded systems
- watching live values from robots, spacecraft, or test rigs
- inspecting low-level comms like serial, CAN bus, or network traffic

## Current Capabilities

Today Prismo includes:

- nested channel trees with collapsible namespaces
- support for multiple plugins at once, with a unified view of all their telemetry
- live rate calculation and stale state markers
- numeric history charting and text/bytes renderers
- vim-like navigation and filter prompts
- mouse interaction in scrollable panes
- OSC 52 clipboard copy support

The docs currently focus on local use, plugin development, and the in-repo
architecture.

## Read This Next

- [Getting Started](./getting-started.md) to start using Prismo quickly locally
- [Architecture](./ARCHITECTURE.md) to understand the runtime and plugin model
- [Development](./DEVELOPMENT.md) to extend the project or work on the codebase
