## Overview
`core::mcp::auth` inspects configured MCP servers and reports whether each target is ready to authenticate. It bridges Codex configuration (`McpServerConfig`) with the RMCP client’s credential helpers so higher layers can surface login prompts or warnings to the user.

## Detailed Behavior
- Defines `McpAuthStatusEntry`, bundling the original `McpServerConfig` with the derived `McpAuthStatus` for UI consumers.
- `compute_auth_statuses`:
  - Accepts an iterator of configured servers plus the desired credential store mode (filesystem vs. keychain).
  - Launches concurrent checks with `join_all`, cloning the name/config pairs so async tasks own their data.
  - Logs a warning and falls back to `McpAuthStatus::Unsupported` if an individual check fails.
  - Returns a `HashMap<String, McpAuthStatusEntry>` keyed by server name for quick lookup.
- `compute_auth_status`:
  - Immediately reports `Unsupported` for STDIO transports (no auth handshake).
  - For `StreamableHttp`, delegates to `determine_streamable_http_auth_status`, forwarding custom headers, bearer token environment bindings, and the credential store selection.

## Broader Context
- Invoked by environment/context aggregators (`../environment_context.rs.spec.md`) and TUI onboarding flows to show whether each MCP server needs authorization.
- Leverages RMCP client utilities to ensure auth checks stay consistent between the core runtime and the dedicated MCP client crate.

## Technical Debt
- None flagged; the module is purposefully small and defers protocol-specific nuances to `codex_rmcp_client`.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../config_types.rs.spec.md
  - ../environment_context.rs.spec.md
  - ./mod.rs.spec.md
