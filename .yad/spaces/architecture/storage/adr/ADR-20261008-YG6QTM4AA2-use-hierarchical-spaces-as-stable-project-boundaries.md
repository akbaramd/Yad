---
yad: 1
schema: adr
schema_version: 1
id: ADR-20261008-YG6QTM4AA2
title: Use hierarchical Spaces as stable project boundaries
status: accepted
space: architecture/storage
created: 2026-10-08T06:36:39Z
updated: 2026-10-08T06:36:42Z
tags:
- space
- organization
- team
---

# Use hierarchical Spaces as stable project boundaries

## Context

Putting every ADR and document into one flat directory becomes difficult to navigate as a project grows.

## Decision

Use lowercase slash-separated Space identifiers such as architecture/search or facilities/approval. Every Space has tracked .space.yaml metadata. Memory references a Space and formal records are physically grouped under it.

## Consequences

Knowledge remains navigable in Git and searchable by logical project area. Space identifiers are portable across Windows, Linux, and macOS.
