---
yad: 1
schema: memory
schema_version: 1
id: MEM-01M4DCPDT1TMFHV3GQAMV6VTV0
kind: decision
space: architecture/core
subject: Single current truth per authoritative memory subject
status: active
importance: 5
source: agent
created: 2026-10-08T09:14:48Z
updated: 2026-10-08T09:14:48Z
tags:
- memory
- lifecycle
---

For fact, decision, and state memories, the tuple kind+space+subject is now single-current-truth. A different active value is rejected and the agent must use memory supersede, preventing silent contradictory current project truths.
