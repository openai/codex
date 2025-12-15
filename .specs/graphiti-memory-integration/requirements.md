# Requirements Document

## Introduction

This feature integrates the Graphiti service with Codex CLI so that Codex can store and recall useful information across turns. The system must remain safe (opt-in + consent), fast (bounded recall + async ingest), and controllable (workspace/session/global scopes, with global promotion-only by default).

### Goals

- Improve agent continuity by recalling relevant prior context.
- Persist session and workspace knowledge automatically when enabled.
- Allow curated, explicit memory promotion to global scope.

### Non-goals

- Building a UI for browsing the entire graph.
- Guaranteeing all turns are stored (best-effort ingestion).

## Glossary

- **Graphiti**: External service providing knowledge-graph storage and retrieval.
- **Episode**: A stored unit of conversation/history (e.g., a Codex turn) ingested into Graphiti.
- **Memory**: Information retrieved from Graphiti and injected into the model prompt.
- **Memory Directive**: A user-authored prefix (e.g. `preference: …`) that requests auto-promotion into Graphiti.
- **Scope**: The grouping boundary for memory (`session`, `workspace`, `global`).
- **Actor / Owner**: The user identity associated with the current Codex session/workspace (used to express “my” preferences/assets).
- **Trusted Project**: A Codex project state that permits networked integrations by default.
- **Consent**: A user-configured flag allowing persistence of conversation data.

## Requirements

### Requirement 1 — Safety gating

**User Story:** As a user, I want Graphiti memory to be off by default and gated, so that my data is not stored without consent.

#### Acceptance Criteria

1.1 THE Codex system SHALL default Graphiti integration to disabled.
1.2 WHEN `graphiti.enabled` is false, THE Codex system SHALL NOT make Graphiti network calls automatically.
1.3 WHEN `graphiti.consent` is false, THE Codex system SHALL NOT ingest any conversation data to Graphiti.
1.4 WHEN the active project is untrusted, THE Codex system SHALL disable automatic ingest and recall by default.

### Requirement 2 — Ingest turns (best-effort)

**User Story:** As a user, I want Codex to persist my session/workspace history, so that it can be recalled later.

#### Acceptance Criteria

2.1 WHEN Graphiti ingest is enabled, THE Codex system SHALL enqueue each completed turn for ingestion without blocking the model response path.
2.2 THE Codex system SHALL bound ingestion memory usage (queue size) and apply a deterministic drop policy when full.
2.3 THE Codex system SHALL retry transient ingestion failures with backoff, and SHALL eventually drop after a bounded number of attempts.
2.4 THE Codex system SHALL write episodes into Graphiti under scope-derived group ids for `session` and `workspace` by default.

### Requirement 3 — Recall memories per turn

**User Story:** As a user, I want Codex to recall relevant memories before answering, so that it can respond consistently without repeating work.

#### Acceptance Criteria

3.1 WHEN Graphiti recall is enabled, THE Codex system SHALL query Graphiti using the current turn’s user input as the recall query.
3.2 THE Codex system SHALL inject recalled memory as a single structured system message (e.g., `<graphiti_memory>…</graphiti_memory>`).
3.3 THE Codex system SHALL cap recalled results (count and total size) to avoid prompt bloat.
3.4 THE Codex system SHALL apply a strict timeout to recall and SHALL fail open (continue the turn without memory) if Graphiti is slow or unavailable.

### Requirement 4 — Scopes and promotion

**User Story:** As a user, I want separate session and workspace memories and an optional global memory, so that I can control where information persists.

#### Acceptance Criteria

4.1 THE Codex system SHALL support `session` and `workspace` scopes for ingestion and recall.
4.2 THE Codex system SHALL support a `global` scope that is disabled for automatic ingestion by default.
4.3 THE Codex system SHALL provide a CLI mechanism to write a curated memory into `workspace` or `global` scope.
4.4 THE Codex system SHALL allow configuration to include/exclude each scope for recall.
4.5 WHEN promoting a memory, THE Codex CLI SHALL wrap the promoted content in a structured `<graphiti_episode kind="…">…</graphiti_episode>` template.

### Requirement 5 — Observability and operability

**User Story:** As a user, I want to diagnose Graphiti connectivity and reset local memory groups, so that I can operate the integration confidently.

#### Acceptance Criteria

5.1 THE Codex CLI SHALL provide a command to test Graphiti connectivity (healthcheck and a small smoke test).
5.2 THE Codex CLI SHALL provide a command to report Graphiti configuration status.
5.3 WHEN explicitly requested, THE Codex CLI SHALL be able to purge a scope group.
5.4 WHEN the active project is untrusted, THE Codex CLI SHALL require an explicit override (e.g. `--allow-untrusted`) for commands that contact Graphiti.

### Requirement 6 — Metadata and privacy

**User Story:** As a user, I want the system to store helpful but safe metadata, so that memories are attributable without leaking sensitive paths.

#### Acceptance Criteria

6.1 THE Codex system SHALL avoid storing absolute filesystem paths in group ids or metadata by default.
6.2 THE Codex system SHALL support a hashed group id strategy by default.
6.3 WHEN enabled, THE Codex system SHALL include basic git metadata (branch, commit, dirty) in episode metadata without including file paths.

### Requirement 7 — User identity and ownership context

**User Story:** As a user, I want Graphiti memories to be associated with my identity and ownership relationships, so that asking about “my” preferences/terminology/assets can recall relevant facts across sessions and workspaces.

#### Acceptance Criteria

7.1 WHEN `graphiti.include_system_messages` is enabled, THE Codex system SHALL ingest an ownership context `system` message at most once per Graphiti group describing the Owner and scope identity.
7.2 WHEN `graphiti.user_scope_key` is configured and Global scope is enabled, THE Codex system SHALL derive the Global scope group id from `graphiti.user_scope_key` using the configured group id strategy (so global memory is isolated per user).
7.3 THE Codex system SHALL NOT attempt to automatically discover the user’s email address or other identity for Graphiti by default (identity must be explicitly configured).

### Requirement 8 — Automatic scope selection and auto-promotion (optional)

**User Story:** As a user, I want global (“my”) memory to be recalled only when relevant, and I want a lightweight way to auto-promote key facts without slowing down the agent loop.

#### Acceptance Criteria

8.1 WHEN `graphiti.recall.scopes_mode` is configured as `auto`, THE Codex system SHALL dynamically include Global scope in recall only when the user query indicates user-specific memory (e.g. preferences/terminology), and SHALL otherwise recall from the configured base scopes.
8.2 WHEN `graphiti.auto_promote.enabled` is enabled, THE Codex system SHALL detect supported Memory Directives in user messages and SHALL enqueue an additional Graphiti message containing a `<graphiti_episode kind="…">…</graphiti_episode>` block without blocking the response path.
8.3 WHEN auto-promotion is triggered, THE Codex system SHALL support explicit scope overrides in the directive (e.g. `(global)` / `(workspace)`), otherwise inferring a scope with a default that prefers the least persistent scope when ambiguous.
8.4 THE Codex system SHALL refuse auto-promotion when the directive content appears to contain secrets (e.g. tokens/passwords/private keys) and SHALL log a debug reason.
