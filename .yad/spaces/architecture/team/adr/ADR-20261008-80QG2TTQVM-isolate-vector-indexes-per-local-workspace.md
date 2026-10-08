---
yad: 1
schema: adr
schema_version: 1
id: ADR-20261008-80QG2TTQVM
title: Isolate vector indexes per local workspace
status: accepted
space: architecture/team
created: 2026-10-08T06:36:28Z
updated: 2026-10-08T06:36:31Z
tags:
- team
- worktree
- qdrant
---

# Isolate vector indexes per local workspace

## Context

The same project can exist in multiple clones or worktrees on one machine. Sharing one Qdrant collection would allow one branch to overwrite another branch index.

## Decision

Keep one tracked project_id but generate an ignored workspace_id for each local checkout. Qdrant collection names include both project_id and workspace_id.

## Consequences

Parallel branches and worktrees cannot corrupt each other semantic indexes. Every clone can rebuild its own derived index from the shared .yad source.
