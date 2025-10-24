## Overview
`core::state::turn` tracks per-turn execution metadata. It models the set of running tasks, their cancellation handles, buffered inputs, and outstanding approval requests while maintaining ordering and task kinds.

## Detailed Behavior
- `ActiveTurn` holds an `IndexMap<String, RunningTask>` keyed by submission ID and an `Arc<Mutex<TurnState>>` for shared turn state. `add_task`, `remove_task`, and `drain_tasks` manage the task set, returning whether any tasks remain after removal.
- `TaskKind` enumerates turn categories (`Regular`, `Review`, `Compact`) and provides an HTTP header value for telemetry or API alignment.
- `RunningTask` encapsulates a task’s notification handle, kind, boxed `SessionTask` trait object, cancellation token, abort-on-drop handle, and the `TurnContext` referenced by the task.
- `TurnState` stores two collections:
  - `pending_approvals`: `HashMap` from sub-ID to `oneshot::Sender<ReviewDecision>`, used by `Session::notify_approval`.
  - `pending_input`: buffered `ResponseInputItem`s awaiting dispatch once a task is ready.
- Methods on `TurnState` insert/remove approvals, push/take pending input, and clear both collections. `ActiveTurn::clear_pending` asynchronously acquires the lock and clears the buffers when a turn ends or scheduling resets.

## Broader Context
- `codex.rs` uses these structures when scheduling tasks via `tasks::SessionTask` implementations. The index map preserves insertion order, ensuring bulk operations like `drain_tasks` process tasks deterministically.
- Approval handling must remain in sync with the tool router; when new approval types are added, `TurnState` and `Session::notify_approval` may require updates.
- Context can't yet be determined for multi-tenant turns or batch approvals; the existing map-based tracking can be extended to include metadata such as timestamps if needed.

## Technical Debt
- None observed; the module provides targeted state containers without embedded logic that could drift.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.rs.spec.md
  - ../codex.rs.spec.md
  - ../tasks/mod.spec.md
