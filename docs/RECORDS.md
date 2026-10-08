# Yad Formal Records and Schema Catalog

Yad v0.2.0 treats formal project documentation as **schema-backed Records**.

A Record is a project artifact that has:

- a stable schema id and abbreviation,
- a stable distributed-safe record id,
- structured Markdown sections,
- required/optional sections,
- a schema-defined lifecycle,
- validation before lifecycle transitions,
- explicit supersession history where the schema permits it,
- Git-owned source files,
- and local lexical/semantic indexing.

The generic command surface is:

```text
yad record types
yad record new
yad record list
yad record show
yad record set
yad record validate
yad record transition
yad record supersede
```

ADR remains available through `yad adr ...` as a compatibility/convenience shorthand.

## Built-in catalog

| Abbr. | Schema id | Category | Purpose / reference |
| --- | --- | --- | --- |
| ADR | adr | Architecture | Architecture Decision Record |
| ARCH | architecture-description | Architecture | ISO/IEC/IEEE 42010 architecture description |
| SRS | requirements-specification | Requirements | ISO/IEC/IEEE 29148 software requirements |
| CM | context-map | Domain Design | DDD strategic Context Map |
| BCC | bounded-context-canvas | Domain Design | Bounded Context Canvas |
| UL | ubiquitous-language | Domain Design | DDD Ubiquitous Language glossary |
| DS | domain-story | Domain Discovery | Domain Storytelling artifact |
| ES | event-storming | Domain Discovery | Persisted EventStorming model/session |
| ADC | aggregate-design-canvas | Domain Design | Aggregate Design Canvas |
| DMF | domain-message-flow | Domain Design | Domain message flow across boundaries |
| API | api-contract | Integration | Synchronous API contract / OpenAPI reference |
| EAPI | event-api-contract | Integration | Event API contract / AsyncAPI reference |
| DATA | data-schema | Data | Data schema / data contract |
| BPM | process-model | Process | Business process model / BPMN reference |
| INC | incident | Operations | Operational or engineering incident record |
| RB | runbook | Operations | Operational procedure |
| CDC | core-domain-chart | Domain Design | DDD Core Domain Chart |
| TM | threat-model | Security | Threat model |
| DR | data-repair | Data Operations | Controlled data repair record |
| MIG | migration | Data Operations | Migration plan and execution record |
| RFC | engineering-rfc | Engineering Design | Engineering Request for Comments |
| QR | quality-requirements | Quality | Measurable quality/NFR requirements |
| PM | postmortem | Operations | Incident postmortem |

The canonical built-in YAML files are stored in:

```text
schemas/builtin/
```

and embedded into the Yad binary. A Yad project owns copies under:

```text
.yad/schemas/
```

The project copies are pinned by `.yad/schemas.lock`.

## Schema definition contract

A schema definition contains:

```yaml
id: incident
version: 1
title: Incident Record
abbreviation: INC
category: operations
description: Tracks an incident from detection through closure.
standard: Operational incident management practice
aliases: [inc, incident-report]

id_prefix: INC
directory: incident

initial_status: open
statuses: [open, investigating, mitigated, resolved, closed, reopened]

historical_statuses: []
authoritative_statuses: [resolved, closed]

transitions:
  open: [investigating, mitigated]
  investigating: [mitigated, resolved]
  mitigated: [investigating, resolved]
  resolved: [closed, reopened]
  closed: [reopened]
  reopened: [investigating]

sections:
  - key: summary
    title: Summary
    required: true
    description: Concise description of the incident.
```

Yad validates the schema definition itself before it can be installed or used.

## Identity

Record IDs use the schema `id_prefix`:

```text
ADR-20261008-XXXXXXXXXX
INC-20261008-XXXXXXXXXX
SRS-20261008-XXXXXXXXXX
BCC-20261008-XXXXXXXXXX
```

The date improves readability; the ULID-derived suffix avoids a central sequence and supports parallel branches.

## Record creation

Schema selectors accept the full schema id, abbreviation, id prefix, or alias:

```powershell
yad record new INC "Cycle 9 reservation incident" --space facilities/approval
yad record new incident "Cycle 9 reservation incident" --space facilities/approval
```

Provide sections inline:

```powershell
yad record new INC "Cycle 9 reservation incident" `
  --space facilities/approval `
  --section "summary=Committed reservations remained after the workflow failed." `
  --section "impact=Entitlement was incorrectly reduced for affected applicants." `
  --section "detection=Detected during cycle 10 review." `
  --section "timeline=Incident identified and scope isolated."
```

A value beginning with `@` is loaded from a UTF-8 file:

```powershell
yad record set INC-... timeline "@timeline.md"
```

Use `@@` when a literal value must begin with `@`.

## Lifecycle

The schema owns valid transitions. Yad does not invent them.

```powershell
yad record transition INC-... investigating
yad record transition INC-... mitigated
yad record transition INC-... resolved
yad record transition INC-... closed
```

An invalid transition is rejected.

A record must be schema-valid before lifecycle transition.

## Supersession

Schemas that define a transition to `superseded` can use:

```powershell
yad record supersede SRS-OLD --by SRS-NEW
```

Yad updates both sides:

- old record: `superseded_by`
- replacement: `supersedes`

The original record is retained in Git history and Yad history.

## Validation and indexing

Validate one record:

```powershell
yad record validate INC-...
```

Validate a schema family:

```powershell
yad record validate --schema INC
```

Validate the entire project:

```powershell
yad validate
```

`yad sync` refuses to rebuild the semantic index when formal records are invalid.

All valid formal record types participate in Yad search and indexing.

## Built-in schema upgrades

Existing projects can install built-ins introduced by a newer Yad version:

```powershell
yad schema upgrade
```

The default behavior installs missing built-ins and preserves modified existing schema files.

`--force` deliberately replaces modified built-in schema files with the version embedded in the running Yad binary, then refreshes `schemas.lock`:

```powershell
yad schema upgrade --force
```

Use `--force` intentionally.

## Project-specific schemas

A project can define artifacts not present in the built-in catalog.

Example:

```yaml
id: business-rule
version: 1
title: Business Rule
abbreviation: BR
category: domain
description: An authoritative business rule for this project.
standard: Project-defined
aliases: [br]

id_prefix: BR
directory: business-rule
initial_status: draft
statuses: [draft, active, superseded]
historical_statuses: [superseded]
authoritative_statuses: [active]

transitions:
  draft: [active]
  active: [superseded]
  superseded: []

sections:
  - key: rule
    title: Rule
    required: true
    description: The rule that must hold.

  - key: rationale
    title: Rationale
    required: true
    description: Why the rule exists.
```

Import it:

```powershell
yad schema add .\business-rule.schema.yaml
```

Yad validates it, stores it as:

```text
.yad/schemas/business-rule.schema.yaml
```

and refreshes `schemas.lock`.

It then becomes immediately available:

```powershell
yad record new BR "Manual approval is final" --space facilities/approval
```

## Philosophy

Yad schemas represent **real project artifacts**, not every modeling concept.

For example, an Entity, Value Object, Command, Domain Event, or Aggregate is normally a model element. It becomes part of a formal Yad document through artifacts such as a Bounded Context Canvas, EventStorming model, Aggregate Design Canvas, API Contract, or Architecture Description.

This distinction keeps the schema catalog useful instead of turning every software concept into a separate document type.
