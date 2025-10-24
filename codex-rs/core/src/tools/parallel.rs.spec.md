## Overview
`core::tools::parallel` provides `ToolCallRuntime`, the component that streams tool calls from the model and schedules them serially or in parallel depending on tool capabilities. It wraps dispatcher calls in abortable tasks and normalizes errors into `CodexErr`.

## Detailed Behavior
- `ToolCallRuntime::new` captures the router, session, turn context, diff tracker, and an `RwLock` used to serialize non-parallel-safe tools.
- `handle_tool_call`:
  - Checks `ToolRouter::tool_supports_parallel` for the target tool.
  - Acquires either a read lock (parallel allowed) or write lock (exclusive execution) on `parallel_execution`.
  - Spawns an abort-on-drop task that invokes `ToolRouter::dispatch_tool_call`.
  - Awaits the task and maps outcomes:
    - Successful responses return `Ok(ResponseInputItem)`.
    - `FunctionCallError::Fatal` and other variants become `CodexErr::Fatal` with appropriate messaging.
    - Task join errors (panics, cancellation) surface as fatal errors with debug info.
- Using `AbortOnDropHandle` ensures tasks are cancelled if the runtime is dropped (e.g., turn aborted).

## Broader Context
- `codex.rs::run_turn` uses `ToolCallRuntime` to process streamed tool calls sequentially while still allowing tools flagged as parallelizable to overlap execution.
- This runtime is agnostic of tool behavior; it simply delegates to the router and enforces concurrency limits. Specs for specific tools explain their parallel support flags.
- Context can't yet be determined for priority or rate limiting; today the runtime offers binary parallel vs. serial behavior.

## Technical Debt
- None observed; concurrency control is straightforward and well-contained.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./router.rs.spec.md
  - ../codex.rs.spec.md
