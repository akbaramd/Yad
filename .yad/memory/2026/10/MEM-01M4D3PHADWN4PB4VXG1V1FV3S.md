---
yad: 1
schema: memory
schema_version: 1
id: MEM-01M4D3PHADWN4PB4VXG1V1FV3S
kind: lesson
space: architecture/team
subject: Schema hash must be line-ending independent
status: active
importance: 4
source: agent
created: 2026-10-08T06:37:34Z
updated: 2026-10-08T06:37:34Z
tags:
- git
- windows
- schema
---

A schema lock based on raw bytes failed after a Windows Git checkout changed LF to CRLF. Yad now hashes canonicalized line endings and generates .yad/.gitattributes to force stable text checkout behavior.
