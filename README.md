# Yad

**Yad** is a local-first project memory and structured-knowledge tool for humans and AI agents.

Its purpose is simple:

> A project should not forget what it learned, why it made a decision, what failed before, or what the current truth is.

Yad gives every project its own persistent memory and formal records without requiring an LLM, a hosted API, or token billing.

An AI agent does the semantic reasoning. Yad provides the durable infrastructure around that reasoning:

- remember important project knowledge,
- retrieve it later,
- preserve project experience,
- keep current facts and decisions consistent,
- create formal project records,
- validate those records,
- organize them by project area,
- share them through Git,
- and rebuild semantic search locally on any team member's machine.

---

## Why Yad exists

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

Records are formal project documents with schemas and lifecycle rules.

The MVP currently includes one formal record type:

**ADR — Architecture Decision Record**

An ADR represents an important project decision together with its context and consequences.

Example:

~~~powershell
yad adr new "Manual approval is final" --space facilities/approval --context "Manual approval is an explicit operator decision with business significance." --decision "Approved requests are excluded from automatic evaluation." --consequences "Re-review requires an explicit operator action."
~~~

Then validate and accept it:

~~~powershell
yad adr validate ADR-...
yad adr accept ADR-...
~~~

ADR lifecycle:

~~~text
proposed
   ├── accepted
   │      ├── deprecated
   │      └── superseded
   └── rejected
~~~

More record types can be added later without changing the basic architecture.

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
Decide whether something deserves an ADR

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
| decision | A project decision that matters but does not yet require a formal ADR |
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

# Installation

The current MVP is developed and tested primarily on Windows.

## Requirements

You need:

- Git
- Docker
- Windows PowerShell or PowerShell
- the Yad executable
- the local embedding model

Docker is used for the local Qdrant instance.

---

## Build from source

Yad is written in Rust.

From the Yad repository:

~~~powershell
tools\cargo-msvc.cmd build --release
~~~

The binary is created at:

~~~text
target\release\yad.exe
~~~

---

## Install the CLI

### Local installation from source

After building the release binary, run:

~~~powershell
powershell -ExecutionPolicy Bypass -File .\tools\install.ps1
~~~

By default this installs:

~~~text
%LOCALAPPDATA%\Yad\bin\yad.exe
~~~

and adds that directory to the current user's persistent Windows PATH, so `yad` can be called from any project directory.

For an elevated machine-wide installation:

~~~powershell
powershell -ExecutionPolicy Bypass -File .\tools\install.ps1 -Scope Machine
~~~

Machine scope installs under:

~~~text
%ProgramFiles%\Yad\bin\yad.exe
~~~

### One-line GitHub installation

After the repository is published, the intended bootstrap command is:

~~~powershell
irm https://raw.githubusercontent.com/akbaramd/Yad/main/install.ps1 | iex
~~~

Here:

- `irm` is PowerShell's `Invoke-RestMethod`.
- `|` pipes the downloaded script text to the next command.
- `iex` is `Invoke-Expression`, which executes that script.

Before publishing, replace `akbaramd/Yad` in `install.ps1` with the real GitHub repository.

The bootstrap installer is designed to:

1. resolve the latest GitHub Release,
2. download `yad-windows-x64.zip`,
3. verify its SHA-256 checksum,
4. install `yad.exe` under `%LOCALAPPDATA%\Yad\bin`,
5. add Yad to the user's PATH,
6. install the pinned embedding model when the model Release asset is present,
7. detect Docker and offer automatic Docker Desktop installation through `winget` when it is missing,
8. start local Qdrant when Docker is already ready.

The expected GitHub Release assets are:

~~~text
yad-windows-x64.zip
yad-windows-x64.zip.sha256
yad-model-multilingual-e5-small-int8.zip
yad-model-multilingual-e5-small-int8.zip.sha256
~~~

Generate the release assets with:

~~~powershell
powershell -ExecutionPolicy Bypass -File .\tools\package-release.ps1 -IncludeModel
~~~

For users who prefer not to pipe a remote script directly into `iex`, the inspect-first flow is:

~~~powershell
irm https://raw.githubusercontent.com/akbaramd/Yad/main/install.ps1 -OutFile install-yad.ps1
notepad .\install-yad.ps1
powershell -ExecutionPolicy Bypass -File .\install-yad.ps1
~~~

Verify installation:

~~~powershell
yad --version
yad --help
~~~

---

# Installing the embedding model

Check model state:

~~~powershell
yad model status
~~~

The expected model is:

~~~text
multilingual-e5-small-int8
~~~

If model files were provided manually, install them from a folder:

~~~powershell
yad model install --from C:\Path\To\ModelFiles
~~~

The required model bundle contains:

~~~text
model.onnx
config.json
tokenizer.json
tokenizer_config.json
special_tokens_map.json
~~~

Yad verifies the model bundle before using it.

---

# Start local infrastructure

Start Qdrant:

~~~powershell
yad infra up
~~~

Check it:

~~~powershell
yad infra status
~~~

Stop it:

~~~powershell
yad infra down
~~~

---

# Initialize Yad in a project

Go anywhere inside the Git repository:

~~~powershell
cd C:\Projects\MyProject
~~~

Then:

~~~powershell
yad init --name "My Project"
~~~

Yad discovers the Git root and creates:

~~~text
<git-root>\.yad
~~~

Yad refuses to overwrite an existing project Memory store.

Check project health:

~~~powershell
yad doctor
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
- durable project decisions that do not yet require a formal ADR.

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

Normal Yad search returns current knowledge and excludes archived, rejected, deprecated, and superseded history.

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

When a major decision becomes formal:
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

Use ADRs for significant formal decisions.

After clone/pull or changes to .yad, run yad sync.

Before finishing substantial work, record useful durable knowledge and any handoff needed by the next agent.

Prefer Yad CLI commands over manual edits. The tracked .yad files are the source of truth; Qdrant and .yad/.runtime are derived local state.
~~~

