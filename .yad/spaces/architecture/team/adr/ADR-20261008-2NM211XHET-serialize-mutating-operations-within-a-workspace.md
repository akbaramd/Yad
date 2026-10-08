---
yad: 1
schema: adr
schema_version: 1
id: ADR-20261008-2NM211XHET
title: Serialize mutating operations within a workspace
status: accepted
space: architecture/team
created: 2026-10-08T06:44:20Z
updated: 2026-10-08T06:44:23Z
tags:
- concurrency
- agents
- workspace
---

# Serialize mutating operations within a workspace

## Context

Multiple agents or terminals can issue Yad commands against the same checkout concurrently. Memory and ADR lifecycle operations can read-modify-write the same project state, and sync can race with mutations.

## Decision

All source mutations and index rebuilds in one checkout acquire an OS-backed exclusive lock at .yad/.runtime/write.lock with a bounded wait. The lock is workspace-local, so independent clones and worktrees remain independent.

## Consequences

Two writers on the same checkout cannot silently overwrite each other. Crashed processes release the OS lock automatically. Callers receive a clear project-busy error after the wait timeout. Read-only search and inspection remain concurrent.
