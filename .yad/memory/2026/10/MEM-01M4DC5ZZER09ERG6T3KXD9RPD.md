---
yad: 1
schema: memory
schema_version: 1
id: MEM-01M4DC5ZZER09ERG6T3KXD9RPD
kind: state
space: quality
subject: Stress rerun passed correctness gates
status: active
importance: 5
source: agent
created: 2026-10-08T09:05:49Z
updated: 2026-10-08T09:05:49Z
tags:
- stress
- retrieval
- concurrency
- performance
---

Stress rerun after hardening with 111 total documents and 8 concurrent writers passed all correctness gates: retrieval Hit@1=1.00 and Hit@3=1.00 across Persian/English cases; five unrelated queries returned zero results; lifecycle, exact dedup, malformed-memory detection, stale-index protection, Qdrant-loss lexical fallback/rebuild, team clone isolation, and 8/8 concurrent writes all passed. Initial semantic rebuild was about 4.28s. Remaining major issue: each semantic CLI search still takes about 2.7s because a new process reloads the local embedding model.
