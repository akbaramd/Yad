---
yad: 1
schema: memory
schema_version: 1
id: MEM-01M4DBS3MV4DG25713WRPVERFB
kind: lesson
space: quality
subject: Stress small regression passed
status: active
importance: 5
source: agent
created: 2026-10-08T08:58:47Z
updated: 2026-10-08T08:58:47Z
tags:
- stress
- retrieval
- concurrency
---

Stress regression after hardening passed on 111 documents: Hit@1=1.00, Hit@3=1.00, all unrelated Persian/English queries returned zero results, exact duplicate memory deduplicated to one active record, lifecycle filtering passed, malformed memory was detected by validate, 8/8 concurrent writers succeeded, stale index remained stale after external source drift plus local mutation, Qdrant loss fallback/rebuild passed, and a cloned workspace rebuilt an isolated vector collection successfully.
