---
yad: 1
schema: adr
schema_version: 1
id: ADR-20261008-NE1HJRAPN1
title: Keep Yad core LLM-free
status: accepted
space: architecture/core
created: 2026-10-08T06:36:05Z
updated: 2026-10-08T06:36:08Z
tags:
- core
- llm
- agent
---

# Keep Yad core LLM-free

## Context

Yad is used by external AI agents that already perform semantic reasoning. Calling another hosted LLM inside Yad would duplicate reasoning, add token cost, and couple project memory to a model vendor.

## Decision

Yad core must not require an LLM. Agents classify and interpret knowledge; Yad validates, stores, versions, indexes, and retrieves it. Local embeddings are allowed because they are retrieval infrastructure, not a reasoning service.

## Consequences

The core remains deterministic and inexpensive. MCP and CLI clients can use any agent. Features that require semantic judgment must be expressed as agent actions rather than hidden Yad inference.
