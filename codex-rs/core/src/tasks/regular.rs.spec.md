## Overview
`core::tasks::regular` defines the standard task runner that powers everyday Codex turns. It delegates to `run_task` with `TaskKind::Regular`, driving the main conversation loop that executes tool calls and aggregates assistant responses.

## Detailed Behavior
- `RegularTask` implements `SessionTask` with `kind()` returning `TaskKind::Regular`.
- `run` clones the underlying `Session` via `SessionTaskContext`, invokes `run_task` with the provided `TurnContext`, user inputs, and cancellation token, and returns the optional final assistant message from the shared loop.
- Task-specific abort handling is not required; the default no-op suffices because regular turns can rely on the generic cancellation path in `Session::handle_task_abort`.

## Broader Context
- This task is spawned for most `UserTurn` operations. Its behavior is governed by `run_task` in `codex.rs`, which covers history recording, tool routing, error handling, and auto-compaction.
- Keeping the wrapper minimal ensures regular turn semantics stay centralized; any changes to turn processing should be made in `run_task`, not here.
- Context can't yet be determined for sub-variants (e.g., streaming vs. non-streaming regular tasks); such differences would likely branch inside `run_task`.

## Technical Debt
- None observed; the module is intentionally lightweight.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.rs.spec.md
  - ./review.rs.spec.md
  - ../codex.rs.spec.md
