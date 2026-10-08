---
yad: 1
schema: memory
schema_version: 1
id: MEM-01M4D3PPSVBDY3XYW9NDCWV2HC
kind: warning
space: architecture/search
subject: Search relevance threshold not implemented
status: active
importance: 4
source: agent
created: 2026-10-08T06:37:40Z
updated: 2026-10-08T06:37:40Z
tags:
- search
- ranking
---

Current search fusion score is a ranking score, not calibrated confidence. There is no semantic relevance threshold yet, so unrelated active documents may appear after the strongest result when the corpus is small.
