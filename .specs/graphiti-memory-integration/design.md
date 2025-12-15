# Design Document: Graphiti Memory Integration (Codex CLI)

## Overview

Codex CLI currently treats each turn as ephemeral: context only persists within the active conversation buffer. This feature integrates the external **Graphiti** knowledge-graph service to:

- **Ingest** Codex conversation turns as messages for long-term storage.
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
    - Optional git metadata formatting for `source_description` (repo/branch/commit/dirty), computed once at session init with bounded timeouts (no per-turn git calls).

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

Each Codex turn is encoded into Graphiti as one or more `/messages` calls, grouped by `group_id`:

- `group_id`: derived from scope (session/workspace/global) and a stable identifier (hashed by default).
- `role_type`: `user` / `assistant` / `system` (Graphiti’s supported role set).
- `content`: the raw text for that message (with small normalization).
- `source_description`: identifies Codex and carries safe metadata (e.g., repository name; optionally git branch/commit/dirty).

Codex does not rely on a Graphiti-side idempotency key; it de-dupes within the current session before enqueueing ingestion jobs.

### Recall query

- Query string is derived from the user inputs for the current turn.
- Search runs with a strict timeout and maximum result count.
- Returned facts are formatted into a single system message with a hard character cap.

### Dynamic recall scope selection (`graphiti.recall.scopes_mode=auto`)

To avoid always querying Global scope (privacy/latency) while still answering “my preferences / my terminology / my owned assets” queries, an optional `auto` mode dynamically decides whether to include Global scope per turn:

- Always include the configured base scopes (default `session` + `workspace`).
- Include `global` only when the user query indicates user-specific memory (e.g. preferences/terminology).

Selection is heuristic, best-effort, and bounded by the same recall timeout/caps.

### Actor identity + ownership context (optional)

When `graphiti.include_system_messages` is enabled, the ingestion layer prepends a one-time `system` message to each active scope/group containing an `<graphiti_episode kind="ownership_context">…</graphiti_episode>` block describing:

- The Owner identity (from explicit config; no automatic email discovery by default).
- The scope identity (session/workspace/global) and a short natural-language ownership statement (“Owner owns this … scope”).
- Optional, safe metadata such as repo basename and git branch/commit/dirty (when enabled).

This context is sent at most once per group to avoid repeated noise and keep ingestion inexpensive.

### Auto-promotion from Memory Directives (optional)

To reduce the friction of manual promotion while keeping safety and performance, an optional auto-promotion mode can detect explicit “Memory Directives” in user messages (e.g. `preference: …`, `terminology (global): …`) and enqueue an additional synthetic message containing a `<graphiti_episode kind="…">…</graphiti_episode>` block.

Key properties:

- **Opt-in** via `graphiti.auto_promote.enabled`.
- **Non-blocking**: promotion is enqueued on the same bounded ingestion pipeline.
- **Scope inference**:
  - Supports explicit scope markers like `preference (global): …` / `decision (workspace): …`.
  - Defaults to a least-persistent scope when ambiguous (prefer `workspace` over `global`).
- **Secret guard**: heuristically refuses to auto-promote when the directive content looks like credentials or private keys.

### Concrete integration entry points

- Session init + gating: `GraphitiMemoryService::new_if_enabled` in `codex-rs/core/src/graphiti/service.rs#L52`
- Session wiring: `codex-rs/core/src/codex.rs#L666`
- Recall injection: `codex-rs/core/src/codex.rs#L2179`
- Turn ingestion enqueue: `codex-rs/core/src/codex.rs#L2224`

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

## Schema & Relations (recommended direction)

The current integration sends raw conversation messages to Graphiti and relies on Graphiti’s default extraction to create facts. For higher-quality long-term memory (especially for coding + project management), the next layer should add **structured episodes** and **domain entities/relations**.

### Suggested “episode kinds”

- `decision` (architecture choice + rationale)
- `lesson_learned` (what went wrong/right; guardrails)
- `procedure` (repeatable steps/runbooks)
- `preference` (style/standards, repo conventions)
- `task_update` (status + blockers + next steps)

Codex already supports explicit curated episodes via `codex graphiti promote`, which wraps content in a `<graphiti_episode kind="…">` template.

### Suggested entities and relations

These are intentionally “generic coding agent” primitives; they can be added later either by:
1) emitting structured episode content, or
2) extending Graphiti’s extraction/ontology configuration.

Entities (examples): `Repository`, `Branch`, `Commit`, `PullRequest`, `File`, `Symbol`, `Task`, `Decision`, `Incident`, `Tool`, `Service`.

Relations (examples):
- `APPLIES_TO` (Preference/Lesson → Repository/Workspace)
- `HAPPENED_ON` (Incident → Branch/Commit)
- `MODIFIES` (Commit/PR → File/Symbol)
- `IMPLEMENTS` (Commit/PR → Task/Requirement)
- `BLOCKS` / `DEPENDS_ON` (Task → Task)
- `USES_TOOL` (Procedure/Turn → Tool)
- `PREFERS` (User → Tool/Style/Workflow)
- `ALIAS_OF` / `TERMINOLOGY_FOR` (Term → Term/Concept)
- `CREATED` / `AUTHORED` (User → PR/Commit/Decision)

Privacy note: prefer repo-relative identifiers and hashed ids over absolute paths.

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
