## Overview
`lib.rs` presents the rmcp-client crate’s public API. It re-exports the MCP client, OAuth helpers, and auth-status utilities so downstream crates can depend on this crate without touching internal modules.

## Detailed Behavior
- Re-exports:
  - `RmcpClient` for stdio/HTTP MCP sessions.
  - OAuth helpers (`perform_oauth_login`, `save_oauth_tokens`, `delete_oauth_tokens`, etc.) and `OAuthCredentialsStoreMode` / `StoredOAuthTokens` types.
  - `determine_streamable_http_auth_status` and `supports_oauth_login` for auth detection.
  - `McpAuthStatus` from `codex_protocol` for compatibility.
- Keeps module organization private, exposing only the functions/types required by consumers such as `codex-core`.

## Broader Context
- Serves as the boundary between Codex and the RMCP SDK, allowing other crates to handle MCP connections without bundling the entire implementation detail.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./rmcp_client.rs.spec.md
