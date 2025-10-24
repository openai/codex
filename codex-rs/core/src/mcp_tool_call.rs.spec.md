## Overview
`core::mcp_tool_call` handles the lifecycle of MCP tool invocations. It emits begin/end events, parses arguments, invokes MCP servers via the session, and packages results into protocol responses for the model.

## Detailed Behavior
- `handle_mcp_tool_call`:
  - Parses tool call arguments as JSON, allowing empty strings but returning an error `FunctionCallOutputPayload` when parsing fails.
  - Emits `McpToolCallBegin` with the server/tool/arguments metadata so UIs can display pending MCP work.
  - Invokes `Session::call_tool`, capturing the duration and logging any errors.
  - Emits `McpToolCallEnd` with the invocation metadata, elapsed time, and the result (`Ok(CallToolResult)` or `Err(String)`).
  - Returns a `ResponseInputItem::McpToolCallOutput` containing the result, propagating success/failure to the model stream.
- `notify_mcp_tool_call_event` is a small helper that forwards events via `Session::send_event`.

## Broader Context
- MCP tools are configured via the tool router; this function runs after routing resolves server/tool and ensures telemetry and protocol events remain synchronized.
- The begin/end events allow clients to display progress bars or tool output panes distinct from shell/apply-patch flows.
- Context can't yet be determined for streaming MCP results; current behavior assumes a single result payload per call.

## Technical Debt
- None observed; the function cleanly handles event emission, error fallback, and response packaging.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./tools/router.rs.spec.md
  - ./tools/context.rs.spec.md
  - ../protocol/src/protocol.rs.spec.md
