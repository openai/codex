## Overview
`core::tools::handlers::mcp_resource` implements the trio of MCP resource helpers: listing resources, listing resource templates, and reading a specific resource. It wraps MCP client calls with structured payloads, emits begin/end events, and normalizes output into JSON for the model.

## Detailed Behavior
- Accepts only `ToolPayload::Function`; the router sets the tool name to one of `list_mcp_resources`, `list_mcp_resource_templates`, or `read_mcp_resource`.
- `parse_arguments` handles optional JSON (treating whitespace as no arguments) and deserializes into tool-specific argument structs, normalizing optional strings.
- For each operation:
  - Builds an `McpInvocation`, emits `McpToolCallBegin`, and records the start time.
  - Invokes MCP client methods:
    - `list_resources`: either per server (respecting cursors) or across all servers via `McpConnectionManager::list_all_resources`.
    - `list_resource_templates`: similar logic with template-specific requests.
    - `read_resource`: fetches a specific resource by URI.
  - Wraps results in payload structs (`ListResourcesPayload`, `ListResourceTemplatesPayload`, `ReadResourcePayload`) that include server names, cursors, and resource metadata. Payloads are sorted deterministically when aggregating across servers.
  - Serializes payloads to JSON (surface-level errors bubble up as model responses).
  - Emits `McpToolCallEnd` with duration and either a `CallToolResult` (success) or error string.
- Returns `ToolOutput::Function` containing the serialized JSON and `success` flag that mirrors the MCP call outcome.

## Broader Context
- These handlers provide Codex-native wrappers over MCP resource APIs, allowing models to discover and consume MCP resources before deciding to invoke other tools.
- Because results can span multiple servers, payloads include the server name alongside each resource/template to maintain clarity for the model.
- Context can't yet be determined for streaming or paginated aggregate responses; current behavior requires callers to supply cursors when targeting specific servers.

## Technical Debt
- None explicitly noted; argument parsing and payload serialization are comprehensive.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.rs.spec.md
  - ../spec.rs.spec.md
  - ../../mcp_tool_call.rs.spec.md
