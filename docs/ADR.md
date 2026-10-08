# ADR record contract

ADR remains a built-in formal record schema: `adr@1`. Since Yad v0.2.0 it runs on the same Generic Record Engine as the other built-in and project-specific schemas.

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

## Generic Record compatibility

The dedicated ADR commands remain available as a convenient shorthand:

```powershell
yad adr new
yad adr list
yad adr validate
yad adr accept
yad adr reject
yad adr deprecate
yad adr supersede
```

The same ADR schema can also be addressed by the generic engine:

```powershell
yad record new ADR "Decision title" --space architecture/core
yad record list --schema ADR
yad record validate --schema ADR
```

See `docs/RECORDS.md` for the complete schema catalog and Generic Record Engine.
