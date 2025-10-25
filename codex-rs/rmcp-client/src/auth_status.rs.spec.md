## Overview
`auth_status.rs` determines whether a streamable HTTP MCP server requires OAuth login, already has stored tokens, or supports bearer token auth. It probes backend metadata and consults local credential stores to return an `McpAuthStatus` classification.

## Detailed Behavior
- `determine_streamable_http_auth_status`:
  - Returns `BearerToken` immediately when an env var supplies the token.
  - Checks local storage (`has_oauth_tokens`) for cached tokens; if present returns `OAuth`.
  - Otherwise builds default headers (static plus env-sourced), issues discovery requests via `supports_oauth_login_with_headers`, and returns `NotLoggedIn` when endpoints advertise OAuth or `Unsupported` when they do not or probing fails.
- `supports_oauth_login` / `_with_headers` send GET requests to well-known OAuth discovery paths (RFC 8414), including a `MCP-Protocol-Version` header. Success requires a 200 OK with JSON containing both `authorization_endpoint` and `token_endpoint`.
- `discovery_paths` constructs candidate URLs based on the server’s base path to handle nested deployments.
- Logging uses `tracing::debug` to record failures without surfacing them as hard errors.

## Broader Context
- Called from higher-level tooling to preflight OAuth requirements before attempting login or connecting via `RmcpClient::new_streamable_http_client`.

## Technical Debt
- None; future MCP protocol revisions may add additional discovery headers, but the helper is easy to extend.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./oauth.rs.spec.md
