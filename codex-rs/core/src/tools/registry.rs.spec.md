## Overview
`core::tools::registry` registers tool handlers and dispatches invocations. It ties together handler implementations, telemetry logging, and tool specs used by the router to expose available tools to the model.

## Detailed Behavior
- `ToolKind` distinguishes function vs. MCP handlers, and `ToolHandler` exposes the execution contract (`handle`) plus helper methods (`kind`, `matches_kind`) to guard against payload mismatches.
- `ToolRegistry` stores handlers keyed by tool name. `dispatch`:
  - Retrieves the handler and emits an early `FunctionCallError::RespondToModel` if no handler exists.
  - Validates payload compatibility via `matches_kind`; mismatches produce fatal errors and telemetry entries.
  - Wraps handler execution with `otel_event_manager.log_tool_result`, capturing duration, previews, and success flags. Handler outputs are stored in a mutex to produce the final `ResponseInputItem` once telemetry logging completes.
  - Converts `ToolOutput` into protocol responses using `ToolOutput::into_response`, preserving custom vs. function output types.
- `ToolRegistryBuilder` prepares the registry:
  - `push_spec` / `push_spec_with_parallel_support` collect `ConfiguredToolSpec` entries so the router can expose tool metadata and parallel capability flags.
  - `register_handler` adds handlers, warning when overwriting existing ones (dynamic tool registration is TODO’d but not yet implemented).
  - `build` returns the final `ToolRegistry` and the list of configured specs.
- `ConfiguredToolSpec` embeds a `ToolSpec` plus a parallel-support flag consumed by the router and orchestrator when scheduling tool calls.

## Broader Context
- Handlers under `tools::handlers` implement `ToolHandler` and register via the builder inside `spec.rs`. New tools must register here to be available to the router.
- Telemetry integration ensures every tool call captures success/failure along with sanitized payload snippets (`ToolPayload::log_payload`). This keeps downstream observability consistent.
- Context can't yet be determined for dynamic tool loading; builder TODOs suggest eventual runtime registration, which would require additional APIs.

## Technical Debt
- Dynamic registration TODOs remain; without them, adding/removing tools requires code changes. Future work should either remove the TODOs or implement the registration APIs.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Decide on dynamic tool registration support and either implement or delete the commented APIs to avoid lingering TODOs.
related_specs:
  - ./mod.rs.spec.md
  - ./router.rs.spec.md
  - ./handlers/mod.rs.spec.md
