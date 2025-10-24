## Overview
`core::tools::handlers::mcp` dispatches general MCP tool calls. It unwraps the payload produced by the router, forwards execution to `handle_mcp_tool_call`, and converts the result into the appropriate `ToolOutput`.

## Detailed Behavior
- Accepts only `ToolPayload::Mcp { server, tool, raw_arguments }`; any other payload yields a model-facing error.
- Calls `handle_mcp_tool_call`, which emits begin/end events, invokes the MCP server, and returns a `ResponseInputItem`.
- Depending on the response variant:
  - `McpToolCallOutput` is converted into `ToolOutput::Mcp` preserving the success/error result for downstream telemetry.
  - `FunctionCallOutput` (fallback path when errors are surfaced as function outputs) is converted into `ToolOutput::Function`.
- Unexpected variants trigger an error so the model can adjust; this guards against schema drift.

## Broader Context
- MCP tool specs are registered via `tools/spec.rs`. This handler allows tools to share a single runtime path while `mcp_resource` handles specialized resource management operations.
- Telemetry and approval flows live in `handle_mcp_tool_call`; the handler simply adapts outputs to the registry contract.
- Context can't yet be determined for streaming MCP responses; current behavior assumes discrete results.

## Technical Debt
- None identified.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.rs.spec.md
  - ../spec.rs.spec.md
  - ../../mcp_tool_call.rs.spec.md
