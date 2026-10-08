---
yad: 1
schema: memory
schema_version: 1
id: MEM-01M4D433ZYB710N4HKB9Q1QHQG
kind: lesson
space: architecture/team
subject: Windows file-lock contention semantics
status: active
importance: 4
source: agent
created: 2026-10-08T06:44:26Z
updated: 2026-10-08T06:44:26Z
tags:
- windows
- concurrency
---

On Windows, fs2 try_lock_exclusive reports lock contention using OS errors 32/33 rather than always mapping to ErrorKind::WouldBlock. Yad now recognizes both Windows lock codes and WouldBlock so project-busy behavior is portable.
