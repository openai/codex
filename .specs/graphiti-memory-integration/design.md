# Design Document: Graphiti Memory Integration (Codex CLI)

## Overview

Codex CLI currently treats each turn as ephemeral: context only persists within the active conversation buffer. This feature integrates the external **Graphiti** knowledge-graph service to:

- **Ingest** Codex conversation turns as “episodes” for long-term storage.
- **Recall** relevant memories at the start of each turn to improve continuity and reduce repeated explanations.

### Goals

- Add **workspace** and **session** scoped memory (default), plus an optional **global** scope (promotion-only by default).
- Keep the agent loop fast: **non-blocking ingestion**, bounded queues, and **time-bounded recall**.
- Keep it safe: **opt-in**, **explicit consent**, and **trusted-project gating** by default.
- Provide a small CLI surface to **test connectivity** and **promote curated memories**.

### Non-goals

- Replacing Codex’s existing conversation context management.
- Building a full knowledge management UI.
- Implementing domain-specific relation extraction inside Codex (we rely on Graphiti’s default extraction for now).

## Current Architecture

- The Codex runtime builds a turn prompt from the current conversation and submits it to the model.
- There is no durable memory system beyond the active session.

Key paths:
- Turn orchestration: `codex-rs/core/src/codex.rs`
- Session services container: `codex-rs/core/src/state/service.rs`
- Configuration: `codex-rs/core/src/config/…`

## Proposed Architecture

Introduce a best-effort memory subsystem:

```
User input ─┐
            ├─> Build prompt ──> Model turn ──> Assistant output
Graphiti    │                      │                │
Recall  ────┘                      └──── Ingest ─────┘
 (bounded, time-limited)                  (async, queued)
```

### High-level flow per turn

1. **Recall**: before sending a request to the model, call Graphiti search and inject a single system message like:
   - `<graphiti_memory>…facts…</graphiti_memory>`
2. **Ingest**: after a turn completes, enqueue the turn (user input + assistant output + metadata) for background ingestion to Graphiti.

## Components

### `codex-core`

- `graphiti::client` (`codex-rs/core/src/graphiti/client.rs`)
  - REST client for Graphiti endpoints (`/healthcheck`, `/messages`, `/search`, optional group helpers).
  - Uses explicit timeouts and returns structured errors.

- `graphiti::service` (`codex-rs/core/src/graphiti/service.rs`)
  - `GraphitiMemoryService` encapsulates:
    - Gating (enabled + consent + trusted project).
    - Scope → group id mapping (session/workspace/global).
    - Recall formatting and token/length caps.
    - Background ingestion queue + retry policy.
    - Optional git metadata formatting for `source_description` (branch/commit/dirty), computed at session init with bounded timeouts.

- Turn integration (`codex-rs/core/src/codex.rs`)
  - Inserts recall system message before the last user message (so it conditions the reply without overriding user intent).
  - Enqueues ingestion after the assistant output is produced.

### `codex-cli`

- `codex graphiti …` commands (`codex-rs/cli/src/main.rs`)
  - `test-connection`: verifies endpoint reachability and optional “smoke” calls.
  - `status`: reports configured enablement + optional healthcheck.
  - `promote`: writes a curated memory into workspace/global scope.
  - `purge`: removes a group (primarily for local dev; gated).

## Data & Control Flow

### Episode representation (turn ingestion)

Each Codex turn is encoded into Graphiti as one or more “messages” with metadata:

- `group_id`: derived from scope (session/workspace/global) and a stable identifier (hashed by default).
- `role_type`: `user` / `assistant` / `system` (Graphiti’s supported role set).
- `content`: the raw text for that message (with small normalization).
- `source_description`: identifies Codex and carries safe metadata (e.g., repository name; optionally git branch/commit/dirty).
- `idempotency_key`: `${group_id}:${turn_id}` to reduce duplicates on retry.

### Recall query

- Query string is derived from the user inputs for the current turn.
- Search runs with a strict timeout and maximum result count.
- Returned facts are formatted into a single system message with a hard character cap.

## Integration Points

### Configuration

Codex config adds a `[graphiti]` section (see `docs/config.md` and `docs/example-config.md`) including:

- `enabled`, `consent`, `endpoint`
- ingest: queue size, timeouts, enabled scopes
- recall: enablement, timeouts, scopes, max facts, formatting caps
- group id strategy: `hashed` (default) or `raw`
- optional git metadata inclusion

### Trusted project gating

All automatic recall/ingest is disabled unless:

- Graphiti is enabled,
- user consent is true, and
- the active project is trusted.

CLI operations that can bypass trust checks (e.g., connectivity tests) require an explicit `--allow-untrusted` flag.

## Migration / Rollout Strategy

- Ship behind default-off config (`enabled=false`, `consent=false`).
- Provide a demo guide and a `codex graphiti test-connection` command.
- Consider later adding a TUI toggle and/or project-level onboarding prompt.

## Performance / Reliability / Security / UX Considerations

- **Performance**: recall is bounded by timeout and caps; ingestion is async and uses a bounded queue (drop-oldest).
- **Reliability**: fail-open — Graphiti failures do not fail the Codex turn.
- **Security/Privacy**:
  - opt-in + explicit consent,
  - trusted-project gating by default,
  - hashed group identifiers by default to avoid leaking path-like identifiers,
  - avoid including absolute paths in stored metadata.
- **UX**: recalled memory is injected in a structured tag (`<graphiti_memory>`) for predictability and debuggability.

## Risks and Mitigations

- **Memory quality / hallucinations**: Keep recall formatting conservative and include “treat as hints” language.
- **Over-recall**: cap results and total size; scope recall to workspace/session by default.
- **Privacy leakage**: require consent + trusted projects; default to hashed group ids; keep metadata minimal.
- **Slow turns**: tight recall timeout, skip on slow Graphiti responses; ingestion off the hot path.

## Future Enhancements

- Add explicit domain entities/relations (Repository/Branch/Commit/File/Symbol/Task) via richer episode payloads.
- Add a “promotion” workflow: promote frequently-reused workspace memories to global.
- Add de-duplication across sessions and/or a local cache.
- Add a TUI inspector for recalled memories and stored episodes.

