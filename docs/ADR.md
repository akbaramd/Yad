# ADR record contract

The MVP ships one formal record schema: `adr@1`.

Example:

```markdown
---
yad: 1
schema: adr
schema_version: 1
id: ADR-20261008-XXXXXXXXXX
title: Manual approval is final
status: accepted
space: facilities/approval
created: 2026-10-08T00:00:00Z
updated: 2026-10-08T00:00:00Z
tags:
  - workflow
supersedes: []
superseded_by: []
---

# Manual approval is final

## Context

...

## Decision

...

## Consequences

...
```

The schema contract controls required sections and valid lifecycle transitions. Yad validates the record before state changes.

ADR identifiers are distributed-safe rather than sequential so parallel branches do not compete for the same next number.
