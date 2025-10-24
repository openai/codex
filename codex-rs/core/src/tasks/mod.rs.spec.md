## Overview
`core::tasks` orchestrates task execution within a turn. It defines the `SessionTask` trait implemented by the regular, review, and compact task runners, wires cancellation and completion semantics, and integrates with `Session` to manage active tasks.

## Detailed Behavior
- Re-exports `RegularTask`, `ReviewTask`, and `CompactTask` for use by `codex.rs`.
- `SessionTaskContext` wraps an `Arc<Session>` to provide shared access for task implementations without exposing additional internals.
- `SessionTask` requires tasks to report their `TaskKind`, implement `run`, and optionally override `abort`. The default `abort` is a no-op.
- `Session::spawn_task`:
  - Aborts existing tasks before starting a new one, enforcing single-task-at-a-time semantics.
  - Wraps the task in `Arc<dyn SessionTask>`, sets up cancellation tokens and `Notify` handles, and spawns an async runner that calls `Session::on_task_finished` when `run` completes without cancellation.
  - Registers the running task in `ActiveTurn` so approvals and pending input are associated with the correct submission ID.
- `Session::abort_all_tasks` drains `RunningTask`s and delegates to `handle_task_abort`, emitting `TurnAborted` events.
- `handle_task_abort` cancels the task’s token, waits briefly for graceful shutdown, aborts the tokio handle if needed, invokes the task’s `abort` hook, and emits a `TurnAborted` event with the provided reason.
- `on_task_finished` removes the task entry, clears `ActiveTurn` if empty, and emits `TaskComplete` with the optional final assistant message returned from `run`.
- Utility helpers manage the active-turn map (`register_new_active_task`, `take_all_running_tasks`) and ensure `TurnState` pending buffers are cleared when draining tasks.

## Broader Context
- Task runners are invoked by user operations (`RegularTask`) or specialized flows (`CompactTask` for auto-compaction, `ReviewTask` for structured code reviews). Specs for those modules describe their behavior in more detail.
- Cancellation and abort semantics tie into approval workflows and UI feedback. UIs rely on emitted events (`TurnAborted`, `TaskComplete`) to update status, so maintaining these lifecycles is critical.
- The wrapper-based design allows additional task types (e.g., project documentation) to be introduced by implementing `SessionTask`.

## Technical Debt
- The fixed 100 ms grace period for task shutdown is hard-coded; making it configurable or adaptive could improve responsiveness for long-running tasks.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Expose a configurable timeout for graceful task interruption to accommodate tasks with known teardown costs.
related_specs:
  - ./regular.rs.spec.md
  - ./review.rs.spec.md
  - ./compact.rs.spec.md
  - ../codex.rs.spec.md
