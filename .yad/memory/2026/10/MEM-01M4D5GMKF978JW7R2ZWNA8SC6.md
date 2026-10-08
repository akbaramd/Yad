---
yad: 1
schema: memory
schema_version: 1
id: MEM-01M4D5GMKF978JW7R2ZWNA8SC6
kind: failure
space: quality
subject: Stress run 2026-10-08 baseline
status: active
importance: 5
source: agent
created: 2026-10-08T07:09:18Z
updated: 2026-10-08T07:09:18Z
tags:
- stress
- retrieval
- concurrency
- validation
---

Stress run with 100 synthetic noise memories plus 10 gold memories found: retrieval Hit@1=0.80 and Hit@3=1.00; median semantic CLI search latency about 2.74s; unrelated queries still return results with top semantic scores up to about 0.789; exact duplicate remembers are stored twice; malformed Memory is not detected by yad validate; lifecycle stress assertion failed and concurrency harness reported 0/8 writers, both requiring root-cause analysis. Qdrant loss fallback/rebuild and team clone isolation both passed.
