---
yad: 1
schema: adr
schema_version: 1
id: ADR-20261008-YVMF1MERH8
title: Formal records are schema-locked and lifecycle-controlled
status: accepted
space: architecture/storage
created: 2026-10-08T06:36:33Z
updated: 2026-10-08T06:36:36Z
tags:
- schema
- adr
- integrity
---

# Formal records are schema-locked and lifecycle-controlled

## Context

Formal project documents need stronger guarantees than free-form notes. A schema changed silently by one team member could alter validation semantics for everyone.

## Decision

Each formal record type has a versioned schema. schemas.lock pins schema version and canonical content hash. ADR lifecycle transitions are enforced by the schema, and invalid ADRs cannot change lifecycle state or enter the semantic index.

## Consequences

Formal records remain auditable and deterministic across clones. Line ending normalization prevents false schema drift on Windows.
