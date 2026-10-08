---
yad: 1
schema: memory
schema_version: 1
id: MEM-01M4D3PM3N2A8SZMYWEJD1VR32
kind: failure
space: architecture/search
subject: Embedding model bootstrap failure
status: active
importance: 4
source: agent
created: 2026-10-08T06:37:37Z
updated: 2026-10-08T06:37:37Z
tags:
- embedding
- bootstrap
---

FastEmbed internal HuggingFace model download stalled and left lock/incomplete files on MateServer. Yad now supports installing a pinned multilingual-e5-small INT8 model from local files, verifies SHA-256 for all required assets, and uses its own process lock and atomic copy/download path.
