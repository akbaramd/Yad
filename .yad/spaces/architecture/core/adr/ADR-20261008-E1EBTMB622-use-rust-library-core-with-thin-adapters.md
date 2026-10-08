---
yad: 1
schema: adr
schema_version: 1
id: ADR-20261008-E1EBTMB622
title: Use Rust library core with thin adapters
status: accepted
space: architecture/core
created: 2026-10-08T06:36:17Z
updated: 2026-10-08T06:36:19Z
tags:
- rust
- cli
- mcp
---

# Use Rust library core with thin adapters

## Context

The MVP begins as a command-line tool, while MCP is planned later. Duplicating business logic between CLI and MCP would create inconsistent behavior.

## Decision

Implement Yad as a reusable Rust library containing domain and application behavior. yad.exe is a thin CLI adapter. Future MCP support must call the same core library.

## Consequences

CLI and MCP share validation, lifecycle rules, storage, and retrieval. Rust provides a single deployable executable and strong filesystem/type safety.
