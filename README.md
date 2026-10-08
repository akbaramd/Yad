# Yad

> Local-first project memory and structured knowledge for humans and AI agents.

**Current version:** `v0.2.0` (MVP)
**Platform:** Windows x64  
**Runtime model:** local-first; no hosted LLM or embedding API required

Yad gives a project durable memory: decisions, facts, failures, lessons, current state, handoffs, and formal architecture records that survive across developers, AI agents, branches, and time.

> **The agent thinks. Yad remembers.**

---

# Installation

## Recommended: one-line installer

Once the first GitHub Release assets for `v0.2.0` are published, the recommended Windows installation is:

~~~powershell
irm https://raw.githubusercontent.com/akbaramd/Yad/main/install.ps1 | iex
~~~

The bootstrap installer is designed to:

- download the latest Windows x64 Yad release,
- verify its SHA-256 checksum,
- install `yad.exe` under `%LOCALAPPDATA%\Yad\bin`,
- add Yad to the persistent User PATH,
- install the pinned local embedding model when its release asset is available,
- detect Docker, and
- start the local Qdrant infrastructure when Docker is ready.

> **Current repository status:** the source is published, but the one-line installer requires a GitHub Release with the packaged assets. Until that Release exists, use the source installation below.

## Install from source — available now

### Requirements

- Git
- Rust toolchain
- Visual Studio C++ Build Tools / MSVC
- Docker Desktop

Clone and build:

~~~powershell
git clone https://github.com/akbaramd/Yad.git
cd Yad
.\tools\cargo-msvc.cmd build --release
~~~

Install Yad to your User PATH:

~~~powershell
powershell -ExecutionPolicy Bypass -File .\tools\install.ps1
~~~

Verify:

~~~powershell
yad --version
yad --help
~~~

Expected version:

~~~text
yad 0.2.0
~~~

For an elevated machine-wide installation:

~~~powershell
powershell -ExecutionPolicy Bypass -File .\tools\install.ps1 -Scope Machine
~~~

Machine scope installs to:

~~~text
%ProgramFiles%\Yad\bin\yad.exe
~~~

## Local model and Qdrant

Check the pinned multilingual embedding model:

~~~powershell
yad model status
~~~

The default model is:

~~~text
multilingual-e5-small-int8
384 dimensions
local ONNX inference
~~~

Start local Qdrant:

~~~powershell
yad infra up
yad infra status
~~~

The model and Qdrant index are local derived infrastructure; they are not committed into project Git repositories.

---

# Quick start

Inside any Git repository:

~~~powershell
cd C:\Projects\MyProject
yad init --name "My Project"
~~~

Record durable knowledge:

~~~powershell
yad remember "Approved requests must not be automatically re-evaluated after manual approval." --kind decision --space facilities/approval --subject FacilityWorker --importance 5 --source agent
~~~

Search project memory:

~~~powershell
yad search "Why are manually approved requests excluded from the worker?"
~~~

For AI agents, prefer JSON output:

~~~powershell
yad --json search "facility approval rules"
~~~

After a clone, pull, branch change, or external `.yad` edit:

~~~powershell
yad sync
~~~

Check the project:

~~~powershell
yad doctor
~~~

---

# Why Yad exists

During real development, project knowledge becomes fragmented across developer memory, AI conversations, commit messages, issue trackers, source code, temporary notes, documentation, incidents, architecture discussions, and previous agents.

A new developer or agent often knows **what the code looks like now**, but not:

- why it became this way,
- what was already tried,
- which approaches failed,
- what business rules must not change,
- which decision is current,
- or what work should continue next.

Yad makes that knowledge part of the project itself.

---

# Core model

Yad currently has three main concepts:

1. **Memory**
2. **Records**
3. **Spaces**

## Memory

Memory stores durable project experience and current knowledge.

Examples include facts, lessons, failures, warnings, current state, handoffs, important observations, and project decisions.

Example:

~~~powershell
yad remember "Approved requests must not be automatically re-evaluated after manual approval." --kind decision --space facilities/approval --subject FacilityWorker --importance 5 --source agent
~~~

Memory is not meant to be a raw activity log.

Do not store every command, every temporary implementation detail, or every conversation message.

Store information that another developer or agent would benefit from knowing later.

---

## Records

Records are formal project artifacts backed by explicit schemas. Unlike Memory, a Record has a document type, required sections, a stable ID prefix, a lifecycle, validation rules, and a versioned schema pinned in `.yad/schemas.lock`.

Yad v0.2.0 includes a **generic Record Engine**. Record behavior is no longer hard-coded to ADR. Any installed schema can create, validate, search, transition, supersede, and index formal records.

Discover the available document types:

~~~powershell
yad record types
~~~

or:

~~~powershell
yad schema list
~~~

Schemas can be selected by full id or abbreviation. For example, all of these resolve through the schema catalog:

~~~text
ADR   -> adr
SRS   -> requirements-specification
CM    -> context-map
BCC   -> bounded-context-canvas
ES    -> event-storming
ADC   -> aggregate-design-canvas
INC   -> incident
RB    -> runbook
~~~

### Built-in document catalog

| Abbreviation | Schema | What the document is for |
| --- | --- | --- |
| ADR | `adr` | Significant architecture decisions and their consequences |
| ARCH | `architecture-description` | System/subsystem architecture description and views |
| SRS | `requirements-specification` | Functional and non-functional software requirements |
| CM | `context-map` | DDD bounded-context landscape and relationships |
| BCC | `bounded-context-canvas` | Purpose, responsibility, language, rules, and interfaces of one bounded context |
| UL | `ubiquitous-language` | Canonical domain vocabulary and definitions |
| DS | `domain-story` | Concrete domain scenario told as actors, activities, and work objects |
| ES | `event-storming` | Persisted EventStorming model/session result |
| ADC | `aggregate-design-canvas` | Aggregate boundary, invariants, commands, transitions, and events |
| DMF | `domain-message-flow` | Commands/events/queries flowing between domain boundaries |
| API | `api-contract` | Synchronous API contract and compatibility rules |
| EAPI | `event-api-contract` | Event/message API contract and delivery semantics |
| DATA | `data-schema` | Data contract, structure, constraints, ownership, and evolution |
| BPM | `process-model` | Business process flow, decisions, exceptions, and outcomes |
| INC | `incident` | Operational/engineering incident from detection through closure |
| RB | `runbook` | Repeatable operational procedure with verification and rollback |
| CDC | `core-domain-chart` | Strategic DDD core/supporting/generic domain assessment |
| TM | `threat-model` | Security threats, assets, trust boundaries, mitigations, and residual risk |
| DR | `data-repair` | Controlled data repair with evidence, safety, execution, and verification |
| MIG | `migration` | Data/schema/service migration plan and execution record |
| RFC | `engineering-rfc` | Engineering proposal for structured review before commitment |
| QR | `quality-requirements` | Measurable quality/NFR scenarios, metrics, and thresholds |
| PM | `postmortem` | Reviewed incident learning and follow-up actions |

The schemas are stored as normal YAML files under `schemas/builtin/` in the Yad repository and are copied into a project-owned `.yad/schemas/` directory during initialization or schema upgrade.

### Create a formal record

Example Incident:

~~~powershell
yad record new INC "Facility worker changed manual approvals" --space facilities/approval --section "summary=Worker changed requests after explicit manual approval." --section "impact=Approved applicants observed an incorrect state." --section "detection=Detected during operator review." --section "timeline=Worker ran; status changed; incident was isolated."
~~~

IDs use the document abbreviation:

~~~text
INC-20261008-XXXXXXXXXX
SRS-20261008-XXXXXXXXXX
BCC-20261008-XXXXXXXXXX
ADR-20261008-XXXXXXXXXX
~~~

If required sections are missing, Yad still creates a safe skeleton with explicit placeholders, but the record remains invalid until completed.

Fill or replace a section later:

~~~powershell
yad record set INC-... root_cause "The worker re-evaluated requests that had already received manual approval."
~~~

Large section content can be read from a UTF-8 file:

~~~powershell
yad record set INC-... timeline "@incident-timeline.md"
~~~

Validate and move through the lifecycle defined by the schema:

~~~powershell
yad record validate INC-...
yad record transition INC-... investigating
yad record transition INC-... resolved
yad record transition INC-... closed
~~~

List or search a specific document type:

~~~powershell
yad record list --schema INC
yad search "manual approval incident" --kind incident
~~~

ADR remains a first-class built-in schema and retains the convenient legacy shorthand:

~~~powershell
yad adr new "Manual approval is final" --space facilities/approval --context "Manual approval is an explicit operator decision." --decision "Approved requests are excluded from automatic evaluation." --consequences "Re-review requires explicit operator action."
yad adr accept ADR-...
~~~

### Project-specific schemas

Projects are not limited to the built-in catalog. A team can import its own schema:

~~~powershell
yad schema add .\schemas\business-rule.schema.yaml
~~~

The imported schema is validated, copied into `.yad/schemas/`, and pinned into `.yad/schemas.lock`. It immediately becomes usable through the same generic Record Engine.

Existing Yad projects can install newly shipped built-ins with:

~~~powershell
yad schema upgrade
~~~

Use `--force` only when deliberately replacing modified built-in schema files.

---

## Spaces

Spaces organize knowledge by project area.

Examples:

~~~text
architecture/core
architecture/search
facilities/approval
facilities/capacity
integrations/aps
integrations/ims
security/authentication
~~~

Create a Space:

~~~powershell
yad space add facilities/approval --name "Facility Approval"
~~~

Inspect the structure:

~~~powershell
yad space tree
~~~

A Space is more than a folder. It is a stable logical boundary that Memory and Records can belong to.

---

# Architecture principles

## Git is the source of truth

Project knowledge lives under:

~~~text
.yad/
~~~

and should normally be committed with the project.

Example layout:

~~~text
.yad/
├── project.yaml
├── schemas.lock
├── schemas/
│   └── adr.schema.yaml
├── memory/
│   └── YYYY/
│       └── MM/
│           └── MEM-....md
├── spaces/
│   ├── general/
│   │   └── .space.yaml
│   └── architecture/
│       ├── .space.yaml
│       └── search/
│           ├── .space.yaml
│           └── adr/
│               └── ADR-....md
└── .runtime/
~~~

Tracked project knowledge:

~~~text
.yad/project.yaml
.yad/schemas.lock
.yad/schemas/
.yad/memory/
.yad/spaces/
~~~

Local derived state:

~~~text
.yad/.runtime/
~~~

The runtime directory is intentionally ignored by Git.

---

## Qdrant is not the source of truth

Qdrant is only a local semantic index.

The real knowledge remains in Markdown/YAML files under .yad.

If the Qdrant collection is deleted, project knowledge is still safe.

Rebuild it with:

~~~powershell
yad sync
~~~

or:

~~~powershell
yad index rebuild
~~~

---

## Each checkout has its own vector index

A tracked project has one project identity, but every local clone/worktree receives a separate local workspace identity.

This means two branches or worktrees on the same machine do not overwrite each other's Qdrant index.

---

## Yad contains no LLM

Yad does not call OpenAI, Claude, Gemini, or another hosted LLM to understand project knowledge.

The agent already working on the project performs semantic judgment.

The responsibility boundary is:

~~~text
Agent
-----
Understand language
Reason about the project
Decide what is important
Choose Memory kind
Choose Space
Decide whether knowledge belongs in Memory or a formal Record, and choose the right schema

Yad
---
Validate
Persist
Version
Enforce lifecycle
Prevent contradictory current truths
Index
Embed
Search
Retrieve
Preserve history
~~~

In short:

> **The agent thinks. Yad remembers.**

---

# Local semantic search

Yad uses a local multilingual embedding model:

~~~text
multilingual-e5-small INT8
384 dimensions
ONNX
~~~

It supports semantic retrieval across languages, including Persian and English.

No embedding API is required and no token cost is involved.

The model is installed once in the user's Yad cache and shared between local projects.

Qdrant stores only the derived vectors.

---

# Current Memory kinds

The MVP currently supports:

~~~text
fact
lesson
observation
failure
preference
state
handoff
warning
decision
note
~~~

| Kind | Use it for |
| --- | --- |
| fact | A durable fact that is currently true |
| decision | A durable project decision that does not require a full formal decision record |
| state | Current project/module status |
| lesson | Something learned from experience |
| failure | An approach that failed and should not be repeated blindly |
| warning | A risk or limitation future agents should know |
| handoff | What should continue next |
| observation | Useful non-authoritative observation |
| preference | Stable project/team preference |
| note | General durable knowledge that does not fit another kind |

---

# Current truth and supersession

For authoritative Memory types:

~~~text
fact
decision
state
~~~

Yad protects the concept of a **single current truth** for the same:

~~~text
kind + space + subject
~~~

If an existing current decision says:

~~~text
Retry count is 3.
~~~

do not create another active decision saying:

~~~text
Retry count is 5.
~~~

Instead supersede the old Memory:

~~~powershell
yad memory supersede MEM-... "Retry count is 5."
~~~

The old Memory remains in history while the new Memory becomes current.

Normal search excludes superseded/archived history.

To search historical knowledge:

~~~powershell
yad search "retry policy" --include-history
~~~

---

# Typical workflow

## At the beginning of work

Check Yad:

~~~powershell
yad status
~~~

Search for relevant context before making important changes:

~~~powershell
yad search "facility approval worker behavior"
~~~

For agents, prefer machine-readable output:

~~~powershell
yad --json search "facility approval worker behavior"
~~~

---

## During work

When something durable is learned:

~~~powershell
yad remember "Increasing HTTP timeout did not solve the APS incident because the root cause was connection pool exhaustion." --kind failure --space integrations/aps --subject "APS timeout root cause" --importance 4 --source agent
~~~

When an important formal decision is made:

~~~powershell
yad adr new "Use PostgreSQL as the authoritative database" --space architecture/storage --context "The project needs one durable transactional source of truth." --decision "PostgreSQL is the authoritative transactional store." --consequences "Caches and indexes must be rebuildable from PostgreSQL or tracked project data."
~~~

---

## At the end of substantial work

Record what future agents need to know:

~~~powershell
yad remember "Next step is to add the MCP adapter over the existing Rust core after retrieval hardening." --kind handoff --space product/roadmap --subject "Next implementation step" --importance 5 --source agent
~~~

If current project state changed, update it through supersession instead of creating contradictory state Memory.

---

## After clone or pull

After another team member changes .yad:

~~~powershell
git pull
yad sync
~~~

Sync validates tracked Yad content and rebuilds the local semantic index.

---

# Validation

Validate project knowledge:

~~~powershell
yad validate
~~~

Validate an ADR:

~~~powershell
yad adr validate ADR-...
~~~

Verify schema integrity:

~~~powershell
yad schema verify
~~~

Run a full diagnostic:

~~~powershell
yad doctor
~~~

---

# Search

Basic search:

~~~powershell
yad search "Why did APS timeout changes fail?"
~~~

Limit results:

~~~powershell
yad search "approval rules" --limit 5
~~~

Search a Space:

~~~powershell
yad search "worker" --space facilities/approval
~~~

Search a kind:

~~~powershell
yad search "timeout" --kind failure
~~~

Include historical/superseded knowledge:

~~~powershell
yad search "retry policy" --include-history
~~~

Agent-friendly JSON:

~~~powershell
yad --json search "retry policy"
~~~

Yad combines lexical and semantic retrieval and applies relevance filtering to avoid returning unrelated project knowledge.

---

# Main CLI commands

~~~text
yad init
yad status
yad doctor
yad validate
yad sync

yad remember
yad search

yad memory list
yad memory get
yad memory archive
yad memory supersede

yad adr new
yad adr list
yad adr show
yad adr validate
yad adr accept
yad adr reject
yad adr deprecate
yad adr supersede

yad space add
yad space list
yad space tree
yad space show

yad schema list
yad schema show
yad schema verify

yad index rebuild
yad index status

yad model install
yad model status

yad infra up
yad infra down
yad infra status
~~~

Use:

~~~powershell
yad <command> --help
~~~

for command-specific options.

---

# Team workflow

The intended workflow is:

~~~text
Developer / Agent A
        |
        v
    .yad changes
        |
        v
      Git
        |
   commit / push
        |
        v
Developer / Agent B
        |
      pull
        |
        v
    yad sync
        |
        v
local Qdrant index
~~~

Important:

- commit useful .yad changes,
- do not commit .yad/.runtime,
- do not commit the embedding model,
- do not treat Qdrant as authoritative storage.

---

# What should NOT be stored in Yad

Do not use Yad as an unfiltered dump.

Avoid storing:

- passwords,
- API keys,
- authentication tokens,
- private secrets,
- raw conversation transcripts,
- every command that was executed,
- temporary debug output,
- build artifacts,
- large generated logs,
- information already obvious from the code and unlikely to matter later,
- speculative guesses presented as facts.

A useful rule is:

> **If another competent developer or agent returns next month, would knowing this save time, prevent a mistake, explain a decision, or help continue the work?**

If yes, it probably belongs in Yad.

---

# Current MVP status

Yad's MVP currently provides:

- local-first project Memory,
- structured Markdown/YAML storage,
- Git-based team sharing,
- hierarchical Spaces,
- Memory lifecycle and supersession,
- exact duplicate protection,
- single-current-truth protection for authoritative Memory,
- ADR schema and lifecycle,
- schema lock verification,
- local multilingual semantic embeddings,
- Qdrant vector search,
- hybrid lexical + semantic retrieval,
- relevance filtering,
- history filtering,
- workspace-isolated indexes,
- corruption detection,
- atomic tracked-file writes,
- concurrent writer protection,
- index recovery,
- JSON output for agents,
- stress-tested retrieval and recovery.

The current CLI architecture intentionally keeps the reusable Rust core separate so an MCP adapter can be added later without duplicating project logic.

---

# Documentation

Architecture:

~~~text
docs/ARCHITECTURE.md
~~~

ADR contract:

~~~text
docs/ADR.md
~~~

---

# Project Agent Instruction

The following block is intended to be copied into a project's agent instructions, such as:

- AGENTS.md
- repository AI instructions
- coding-agent rules
- MCP agent instructions
- project/system prompt

Copy the block below as-is and adjust only project-specific Space conventions if necessary.

---

## Yad instruction for AI agents

~~~text
# Project Memory — Yad

This project uses Yad as its persistent project memory and structured knowledge system.

Yad is part of the project's working process, not optional documentation.

## Core rule

Use Yad to preserve durable project knowledge so future agents and developers do not need to rediscover the same context.

The agent performs semantic reasoning. Yad stores, validates, versions, indexes, and retrieves the resulting project knowledge.

Do not use Yad as a raw activity log.

## At the start of substantial work

1. Confirm that the current directory belongs to the intended project.
2. Run:

   yad status

3. Before making an architectural, business-rule, workflow, integration, persistence, security, or behavior-changing decision, search Yad for relevant prior knowledge:

   yad --json search "<relevant question or topic>"

4. If historical decisions may matter, also search with:

   yad --json search "<topic>" --include-history

Do not assume that the current source code explains the reason behind existing behavior. Search Yad first when prior context could affect the decision.

## When to write Memory

Use yad remember when information is durable and likely to help future work.

Suitable Memory includes:

- important facts,
- current project/module state,
- lessons learned,
- failed approaches,
- warnings,
- important observations,
- handoff information,
- durable project decisions that do not require a full formal decision record.

Choose the most accurate Memory kind:

fact
decision
state
lesson
failure
warning
handoff
observation
preference
note

Use a meaningful Space and Subject.

Example:

yad remember "Increasing the HTTP timeout did not solve the APS incident because the real cause was connection pool exhaustion." --kind failure --space integrations/aps --subject "APS timeout root cause" --importance 4 --source agent

## Current truth must not fork silently

For authoritative Memory kinds:

fact
decision
state

do not create a second conflicting active value for the same kind + space + subject.

If the current truth changed, supersede the previous Memory:

yad memory supersede <MEMORY_ID> "<new current value>"

Preserve history instead of silently replacing or duplicating it.

## Formal project artifacts

Do not force every durable artifact into Memory or ADR.

When the work produces a document that should be independently reviewed, versioned, validated, updated over time, or referenced later, choose the closest installed formal Record schema.

Discover document types when needed:

    yad record types

Examples:

- significant architecture decision -> ADR
- requirements specification -> SRS
- DDD context relationships -> CM
- bounded context definition -> BCC
- domain vocabulary -> UL
- EventStorming result -> ES
- aggregate design -> ADC
- API contract -> API or EAPI
- process model -> BPM
- production/engineering incident -> INC
- operational procedure -> RB
- threat analysis -> TM
- controlled data correction -> DR
- migration -> MIG
- engineering proposal -> RFC
- quality/NFR requirements -> QR
- incident learning -> PM

Create and manage formal Records with:

    yad record new <SCHEMA_OR_ABBR> ...
    yad record set <ID> <SECTION> <VALUE>
    yad record validate <ID>
    yad record transition <ID> <STATUS>

Use project-specific schemas when the project has a real recurring artifact not represented by the built-in catalog.

## Formal decisions

Use an ADR when a decision is significant enough that future developers should understand:

- the context,
- the chosen decision,
- and its consequences.

Typical ADR subjects include:

- architecture,
- persistence,
- integration strategy,
- security model,
- major workflow rules,
- infrastructure choices,
- important cross-cutting technical decisions.

Create the ADR with:

yad adr new ...

Validate it:

yad adr validate <ADR_ID>

Only accept it when the decision is genuinely final:

yad adr accept <ADR_ID>

Do not accept incomplete or speculative ADRs.

If a later ADR replaces an accepted ADR, use the ADR supersede lifecycle instead of deleting history.

## Spaces

Keep knowledge organized using stable project Spaces.

Prefer semantic project boundaries such as:

architecture/search
architecture/storage
integrations/aps
facilities/approval
security/authentication
product/roadmap

Create a missing Space when necessary:

yad space add <space>

Do not create arbitrary near-duplicate Space names.

## Search behavior

Normal Yad search returns current knowledge and excludes lifecycle states classified as historical by each installed schema. Memory history is also excluded by default.

Use:

--include-history

only when previous decisions or historical attempts matter.

For agent automation prefer JSON output:

yad --json search "<query>"

## After clone, pull, branch changes, or external .yad edits

Run:

yad sync

This validates Yad project data and rebuilds the local semantic index.

If semantic search is unavailable or the index is stale, do not interpret that as proof that knowledge does not exist. Sync first or use the lexical results that Yad provides.

## Before finishing substantial work

Review what changed.

Record only durable knowledge that future work should retain.

At minimum consider whether the session produced any:

- new decision,
- changed current state,
- important fact,
- failure worth avoiding,
- lesson,
- warning,
- unresolved next step,
- handoff.

If there is an important continuation point, write a handoff Memory.

Example:

yad remember "Next step is to implement the MCP adapter over the existing Rust core after retrieval hardening." --kind handoff --space product/roadmap --subject "Next implementation step" --importance 5 --source agent

## Do not store

Never store credentials or secrets in Yad.

Do not store:

- passwords,
- API keys,
- authentication tokens,
- private secrets,
- raw chat transcripts,
- temporary logs,
- every shell command,
- routine implementation noise,
- large generated output,
- guesses presented as facts.

## Editing .yad

Prefer Yad CLI commands over manually editing Yad-managed files.

Do not edit .yad/.runtime.

The tracked .yad Markdown/YAML files are the project source of truth.

Qdrant is only a rebuildable local index.

## Validation

When changing formal Yad records or before committing significant .yad changes, run:

yad validate

For diagnostics:

yad doctor

## Team behavior

Useful .yad changes should be committed with the project so other developers and agents receive the same memory and formal records.

After receiving .yad changes through Git, run:

yad sync

## Guiding principle

Before important work:
Search Yad.

When durable knowledge is learned:
Remember it.

When current truth changes:
Supersede it.

When a durable artifact becomes formal:
Use the appropriate Yad Record schema.

When a major architecture/design decision becomes formal:
Create an ADR.

Before handing work to the next agent:
Leave the project with enough Memory to continue without rediscovering the same context.
~~~

---

# Short version of the project rule

If a repository needs only a compact rule, use:

~~~text
This project uses Yad for persistent project memory.

Before substantial work or important decisions, search Yad for relevant context using: yad --json search "<topic>"

Record durable facts, decisions, state, lessons, failures, warnings, and handoffs with yad remember. Do not store temporary noise or secrets.

For fact/decision/state changes, supersede the previous Memory instead of creating contradictory active truth.

Use ADRs for significant formal decisions. Use the appropriate schema-backed Record for other formal artifacts (requirements, incidents, context maps, EventStorming, runbooks, migrations, data repairs, threat models, and similar documents). Use yad record types when unsure.

After clone/pull or changes to .yad, run yad sync.

Before finishing substantial work, record useful durable knowledge and any handoff needed by the next agent.

Prefer Yad CLI commands over manual edits. The tracked .yad files are the source of truth; Qdrant and .yad/.runtime are derived local state.
~~~