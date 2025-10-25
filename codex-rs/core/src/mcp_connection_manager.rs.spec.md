## Overview
`McpConnectionManager` owns long-lived `RmcpClient` instances for every MCP server defined in the user configuration. It validates server metadata, launches clients, aggregates their tools/resources, and enforces per-server allowlists so Codex can expose MCP functionality through a single command surface.

## Detailed Behavior
- Constants:
  - `MCP_TOOL_NAME_DELIMITER` (`"__"`) and `MAX_TOOL_NAME_LENGTH` ensure generated tool identifiers remain OpenAI-compatible.
  - Default timeouts (`DEFAULT_STARTUP_TIMEOUT`, `DEFAULT_TOOL_TIMEOUT`) cap handshake and tool-call latency.
- Construction:
  - `new` receives configured servers plus the credential store mode.
  - Validates server names (`is_valid_mcp_server_name`), short-circuits disabled servers (still recording filters), resolves bearer tokens from environment variables, and spawns each active server concurrently with `JoinSet`.
  - Initializes every client (STDIO or Streamable HTTP) with Codex’s MCP capabilities, captures startup/tool timeouts, and tracks errors in `ClientStartErrors`.
  - Fetches tools from all running servers via `list_all_tools`, applies per-server `ToolFilter` rules, and renames tools with `qualify_tools` to avoid collisions (SHA1 hashing when names overflow 64 characters).
- Runtime APIs:
  - `list_all_tools` returns a `HashMap<String, Tool>` keyed by the fully-qualified name (`mcp__<server>__<tool>`).
  - `list_all_resources` and `list_all_resource_templates` concurrently page through every server’s cursor-based listings, deduplicating cursors and logging failures.
  - `call_tool` enforces allow/deny filters before delegating to the underlying `RmcpClient`, preserving optional per-tool timeouts.
  - `read_resource` wraps `RmcpClient::read_resource`, annotating errors with server/URI context.
  - `parse_tool_name` decodes the cached metadata so downstream handlers can map user selections back to `(server, tool)` pairs.
- Supporting helpers:
  - `ToolFilter` stores enabled/disabled sets and exposes `allows` to gate tool invocation.
  - `filter_tools` applies per-server filters up-front when seeding the tool registry.
  - `resolve_bearer_token` validates environment bindings required by HTTP transports, surfacing descriptive errors when values are missing or malformed.
  - Tests cover tool qualification, filter application, and long-name hashing to guard against regressions.

## Broader Context
- The manager powers the MCP tool handler (`tools/handlers/mcp.rs.spec.md`) and orchestrator, making remote tools appear alongside native Codex tools.
- Relies on `config_types` for server definitions (`config_types.rs.spec.md`) and coordinates with `mcp/auth` for authentication status reporting.
- Aggregated tools feed into the broader tool registry described in `tools/mod.rs.spec.md`, ensuring consistent naming across CLI, TUI, and MCP server entrypoints.

## Technical Debt
- None surfaced in the implementation; error handling and filtering are comprehensive for current transports.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mcp/mod.rs.spec.md
  - ./mcp/auth.rs.spec.md
  - ./config_types.rs.spec.md
  - ./tools/mod.rs.spec.md
  - ./tools/handlers/mcp.rs.spec.md
