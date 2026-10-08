---
yad: 1
schema: adr
schema_version: 1
id: ADR-20261008-AJE5Q6RA63
title: Repository .yad is the source of truth
status: accepted
space: architecture/storage
created: 2026-10-08T06:36:11Z
updated: 2026-10-08T06:36:14Z
tags:
- git
- storage
- source-of-truth
---

# Repository .yad is the source of truth

## Context

Project memory and formal records must travel with the code and remain available to every team member without a centralized proprietary service.

## Decision

Tracked Markdown, YAML, and schema files under .yad are authoritative project data. Qdrant and .yad/.runtime are derived local state and must be rebuildable.

## Consequences

Git carries project knowledge. Losing Qdrant does not lose knowledge. Merge conflicts remain visible in normal source control instead of being hidden in an external database.
