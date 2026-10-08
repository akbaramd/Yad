---
yad: 1
schema: memory
schema_version: 1
id: MEM-01M4DBS3PYMBVHXFE2S4CWAT2M
kind: warning
space: architecture/search
subject: CLI semantic search startup latency
status: active
importance: 4
source: agent
created: 2026-10-08T08:58:47Z
updated: 2026-10-08T08:58:47Z
tags:
- performance
- search
- embedding
---

Semantic CLI search latency remains about 2.7 seconds per process because each yad search invocation loads the local ONNX embedding model. This is largely process/model startup cost rather than corpus search cost and should be addressed after large-corpus validation, likely with a persistent local runtime or daemon while keeping CLI as the user-facing adapter.
