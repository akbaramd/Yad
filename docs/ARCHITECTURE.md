# Yad Architecture

## Responsibility boundary

Yad deliberately separates semantic judgment from structural integrity.

**Agent responsibilities**
- understand natural language
- decide what is worth remembering
- classify memory kind
- decide whether knowledge should become a formal ADR
- choose the relevant project space
- reason over retrieved context

**Yad responsibilities**
- validate inputs and schemas
- assign stable identifiers
- persist Markdown/YAML
- preserve lifecycle history
- enforce ADR transitions
- protect schema versions with a lock file
- generate local embeddings
- index and retrieve with Qdrant
- detect stale indexes
- provide machine-readable CLI output

No LLM is required inside Yad.

## Source of truth

The source of truth is the repository-owned `.yad` directory. Qdrant is derived state.

```text
Git / .yad
   |
   +-- Memory Markdown
   +-- ADR Markdown
   +-- Space metadata
   +-- Schema + lock
   |
   v
Yad parser / validator
   |
   v
Local embedding model
   |
   v
Workspace-local Qdrant collection
```

Deleting Qdrant must never destroy project knowledge. `yad sync` reconstructs it.

## Workspace isolation

A project has one tracked `project_id`, but each local clone/worktree gets an ignored `workspace-id` under:

```text
.yad/.runtime/workspace-id
```

The Qdrant collection name contains both identifiers. This prevents two branches checked out on the same machine from corrupting each other's semantic indexes.

## Spaces

A Space is a stable logical project boundary such as:

```text
facilities
facilities/approval
facilities/capacity
infrastructure/database
```

Every Space has a tracked `.space.yaml`. Memory references a Space through metadata. Formal records are physically grouped under the Space.

Space IDs are intentionally portable lowercase identifiers so the same repository behaves consistently on Windows, Linux, and macOS.

## Memory lifecycle

Memory is lightweight project experience, not formal documentation.

Current kinds:

```text
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
```

Current lifecycle:

```text
active -> archived
active -> superseded -> new active memory
```

A supersede operation never silently overwrites the old memory.

## ADR lifecycle

The built-in ADR schema defines:

```text
proposed -> accepted -> deprecated
                    -> superseded
proposed -> rejected
```

Required sections:

- Context
- Decision
- Consequences

Invalid ADRs cannot transition lifecycle state. Invalid drafts are not semantically indexed.

## Schema integrity

`.yad/schemas.lock` pins schema version and canonical content hash. Hashing normalizes line endings, so Windows CRLF checkouts do not create false drift.

`yad schema verify`, `yad validate`, and `yad sync` enforce the lock.

## Search

Search currently uses:

1. lexical matching over the Markdown source
2. local multilingual embedding retrieval from Qdrant
3. reciprocal-rank fusion
4. lifecycle filtering
5. authority weighting

Accepted ADRs receive a higher authority weight than informal memory. Archived, rejected, deprecated, and superseded items are excluded by default and can be requested with `--include-history`.

## Embeddings

Default model:

```text
multilingual-e5-small-int8
384 dimensions
local ONNX inference
```

Queries use the E5 `query:` prefix and indexed passages use the `passage:` prefix.

The model revision and embedding configuration participate in the index fingerprint. Changing the embedding model automatically makes the local index stale.

## Future adapters

The CLI is only an adapter over the reusable Rust library. A future MCP server should call the same core modules rather than duplicate behavior.

```text
CLI ----\
        +--> Yad Core
MCP ----/
```
