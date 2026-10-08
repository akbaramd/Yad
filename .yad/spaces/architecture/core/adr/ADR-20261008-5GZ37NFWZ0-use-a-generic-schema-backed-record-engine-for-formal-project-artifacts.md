---
yad: 1
schema: adr
schema_version: 1
id: ADR-20261008-5GZ37NFWZ0
title: Use a generic schema-backed Record Engine for formal project artifacts
status: accepted
space: architecture/core
created: 2026-10-08T11:22:18Z
updated: 2026-10-08T11:22:18Z
tags:
- records
- schema
- architecture
---

# Use a generic schema-backed Record Engine for formal project artifacts

## Context

Yad originally hard-coded formal records around ADR, but real software projects maintain many independently reviewed artifacts such as requirements specifications, context maps, EventStorming models, aggregate design canvases, incidents, runbooks, API contracts, data repairs, migrations, threat models, and postmortems. Treating each artifact as bespoke Rust code would not scale and would prevent project-specific document types.

## Decision

Formal project artifacts are represented by project-owned schemas and handled by one Generic Record Engine. Schemas define identity prefix, abbreviation, purpose, sections, lifecycle transitions, historical states, and authoritative states. Yad ships a curated built-in catalog while allowing validated project-specific schemas. Modeling concepts such as Entity, Value Object, Command, or Domain Event are not standalone schema types unless they are part of a real document artifact.

## Consequences

All installed schemas share creation, validation, lifecycle, supersession, indexing, retrieval, and Git persistence. ADR remains a compatible shorthand. Projects can evolve their own formal knowledge model without changing Yad Rust code, while schemas.lock protects semantics from silent drift.
