---
yad: 1
schema: memory
schema_version: 1
id: MEM-01M4DCNWWRX9JFJKBC639P5CAX
kind: lesson
space: architecture/storage
subject: Atomic tracked-file persistence on Windows
status: active
importance: 5
source: agent
created: 2026-10-08T09:14:30Z
updated: 2026-10-08T09:14:30Z
tags:
- windows
- durability
- atomic
---

Tracked Yad source files now use atomic write-then-replace. A Windows stress test with simultaneous readers exposed transient MoveFileExW errors 5, 32 and 33; bounded retry was added. The concurrent reader regression now verifies readers only observe complete old or complete new content and never partial/mixed files.
