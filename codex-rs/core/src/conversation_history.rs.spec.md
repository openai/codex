## Overview
`core::conversation_history` maintains the in-memory transcript of a Codex session. It tracks response items, enforces invariants between tool calls and their outputs, and provides utilities for trimming or replacing history during operations like auto-compaction.

## Detailed Behavior
- `ConversationHistory` stores `ResponseItem`s ordered from oldest to newest. `record_items` filters out non-API messages (system notices, `Other` variants) and appends the remaining entries, cloning each item.
- `get_history` normalizes invariants before returning a cloned vector. Normalization enforces:
  - Every `FunctionCall`, `CustomToolCall`, or `LocalShellCall` (with `call_id`) has a corresponding output. Missing outputs trigger `error_or_panic` logging/panic and synthetic “aborted” outputs are inserted immediately after the call.
  - Every output has a matching call; orphan outputs are removed after logging.
- `remove_first_item` drops the oldest entry and removes its paired item when applicable, preserving call/output consistency without re-running full normalization.
- `replace` swaps the underlying vector, and `remove_corresponding_for`/`remove_first_matching` provide targeted cleanup helpers used by compaction routines.
- `normalize_history` orchestrates the invariant passes (`ensure_call_outputs_present` and `remove_orphan_outputs`). These functions scan the history for missing counterparts, recording issues via `error_or_panic`.
- `is_api_message` determines whether a message should be tracked (user/assistant messages, tool calls, reasoning, web search) versus ignored (system messages, `Other`).
- Tests cover filtering of non-API messages, call/output pair enforcement, local shell pairing, and removal of orphan outputs, ensuring the invariants hold across edge cases.

## Broader Context
- `Session` relies on this history to reconstruct prompts, calculate truncations, and persist rollout entries. When tool semantics evolve (e.g., new call types), this module must be updated to maintain pairing invariants.
- Synthetic “aborted” outputs are a safeguard for missing events; downstream components that display history should interpret these placeholders accordingly.
- Context can't yet be determined for multi-modal or streaming tool outputs; additional invariants may be needed when output shapes become more complex.

## Technical Debt
- `error_or_panic` toggles between logging and panicking based on debug builds and package version, which can surprise consumers. Aligning error handling with a structured telemetry path would provide clearer diagnostics.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Replace `error_or_panic` with structured error reporting so production builds record actionable telemetry without panics.
related_specs:
  - ./codex.rs.spec.md
  - ./codex/compact.rs.spec.md
  - ./conversation_manager.rs.spec.md
