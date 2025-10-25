## Overview
`utils.rs` contains helper utilities for the RMCP client: timeout handling, JSON conversions between `mcp-types` and the RMCP SDK, environment construction for stdio servers, and header preparation for HTTP transports.

## Detailed Behavior
- `run_with_timeout` wraps futures with optional `tokio::time::timeout`, decorating errors with descriptive labels.
- `convert_call_tool_result`, `convert_to_rmcp`, and `convert_to_mcp` serialize values through JSON to bridge between the MCP spec structs and the RMCP SDK’s models, ensuring content arrays default to empty vectors.
- `create_env_for_mcp_server` merges default environment variables with user-specified ones from configuration and runtime environment.
- `build_default_headers` and `apply_default_headers` construct `HeaderMap`s from static header overrides and environment variables, logging warnings for invalid names/values.
- Platform-specific `DEFAULT_ENV_VARS` whitelist env vars that should be passed to stdio MCP servers.
- Tests cover diff conversion, env var handling, and header overrides.

## Broader Context
- Used extensively by `RmcpClient` when spawning child processes, building HTTP clients, and converting results; also shared with OAuth and auth-status modules for consistent header behavior.

## Technical Debt
- None noted.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./rmcp_client.rs.spec.md
  - ./oauth.rs.spec.md
