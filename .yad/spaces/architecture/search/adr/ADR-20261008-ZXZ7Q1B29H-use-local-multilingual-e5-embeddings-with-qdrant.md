---
yad: 1
schema: adr
schema_version: 1
id: ADR-20261008-ZXZ7Q1B29H
title: Use local multilingual E5 embeddings with Qdrant
status: accepted
space: architecture/search
created: 2026-10-08T06:36:22Z
updated: 2026-10-08T06:36:25Z
tags:
- embedding
- qdrant
- search
- multilingual
---

# Use local multilingual E5 embeddings with Qdrant

## Context

Yad needs semantic retrieval across Persian and English project knowledge without API tokens or a hosted embedding provider.

## Decision

Use the pinned multilingual-e5-small INT8 ONNX model with 384 dimensions for local embeddings, using E5 query/passages prefixes. Use Qdrant as the local vector index.

## Consequences

Semantic search has zero API token cost and works cross-language. The model files are installed once in the user cache. Model identity and revision participate in the index fingerprint.
