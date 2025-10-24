## Overview
`core::tools::context` defines the shared structures passed between the tool router, registry, and handlers. It standardizes tool invocation payloads, manages diff trackers, and formats tool outputs for telemetry and protocol responses.

## Detailed Behavior
- `SharedTurnDiffTracker` wraps `TurnDiffTracker` in an `Arc<Mutex<...>>` for use across async tasks processing apply-patch or shell commands.
- `ToolInvocation` packages the session, turn context, diff tracker, call ID, tool name, and payload. It is the primary input to `ToolRegistry::dispatch`.
- `ToolPayload` encodes the different tool call shapes (function, custom, local_shell, unified_exec, MCP). `log_payload` returns the string representation logged to telemetry.
- `ToolOutput` captures handler responses:
  - `Function` outputs store content and optional success flags. `into_response` converts to either `FunctionCallOutput` or `CustomToolCallOutput` depending on the original payload.
  - `Mcp` wraps `CallToolResult` or an error string and maps directly into `McpToolCallOutput`.
- `telemetry_preview` truncates tool outputs for telemetry logs, respecting byte and line budgets while appending a truncation notice when output exceeds limits.
- Unit tests ensure payload/output round-tripping, MCP result previews, and truncation behavior align with expectations.

## Broader Context
- Tool handlers use these structures when reporting results or failures. Specs for `router.rs` and `registry.rs` detail how payloads and outputs flow through the dispatcher.
- Telemetry previews align with constants defined in `tools/mod.rs`; keeping these in sync avoids mismatched truncation between logs and model responses.
- Context can't yet be determined for structured payload logging (e.g., JSON redaction); future requirements may extend `log_payload`.

## Technical Debt
- None observed; the module provides focused data structures and conversion helpers.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.rs.spec.md
  - ./router.rs.spec.md
  - ./registry.rs.spec.md
