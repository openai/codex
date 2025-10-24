## Overview
`core::state::session` stores mutable data that persists across turns of a Codex session. It captures the active configuration, conversation history, token usage stats, and latest rate-limit snapshot, replacing what historically lived directly on `Session`.

## Detailed Behavior
- `SessionState::new` seeds the struct with a `SessionConfiguration`, an empty `ConversationHistory`, and cleared telemetry fields.
- History helpers (`record_items`, `history_snapshot`, `clone_history`, `replace_history`) forward to `ConversationHistory`, providing consistent filtering and normalization of response items for other modules.
- Token helpers:
  - `update_token_info_from_usage` folds new `TokenUsage` data into `TokenUsageInfo`, tracking cached vs. non-cached usage relative to the model context window.
  - `set_token_usage_full` forces the tracked usage to show a full context window when compaction or errors dictate the remaining space is zero.
  - `token_info_and_rate_limits` returns clones so callers can expose current stats without mutating the underlying state.
- Rate-limit tracking uses `latest_rate_limits` to hold the most recent snapshot emitted by the backend.
- Pending approvals and buffered input were migrated to `TurnState`, keeping session-scoped data focused on persistent concerns.

## Broader Context
- `SessionState` is guarded by a `Mutex` in `Session`, ensuring consistent reads/writes across async tasks. Modules that mutate history or token info should hold the lock only briefly to avoid blocking event handling.
- The struct depends on `ConversationHistory` invariants; any changes to tool call handling must be mirrored here by calling the appropriate helpers.
- Context can't yet be determined for sharding state across threads; if needed, `SessionState` could be split further into read-heavy and write-heavy sections.

## Technical Debt
- None observed; the struct provides straightforward getters/setters around the persistent session data.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../codex.rs.spec.md
  - ../conversation_history.rs.spec.md
  - ./mod.rs.spec.md
