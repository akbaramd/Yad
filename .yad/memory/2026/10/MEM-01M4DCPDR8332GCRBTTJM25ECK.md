---
yad: 1
schema: memory
schema_version: 1
id: MEM-01M4DCPDR8332GCRBTTJM25ECK
kind: lesson
space: architecture/team
subject: Windows atomic replace contention
status: active
importance: 4
source: agent
created: 2026-10-08T09:14:47Z
updated: 2026-10-08T09:14:47Z
tags:
- windows
- atomic
---

Concurrent readers on Windows can transiently cause MoveFileExW to return ERROR_ACCESS_DENIED (5), not only sharing violations 32/33. Atomic tracked-file replacement now retries errors 5/32/33 for a bounded interval before failing.
