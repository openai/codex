## Overview
`perform_oauth_login.rs` runs the interactive OAuth authorization flow for MCP streamable HTTP servers. It spins up a local callback server, launches the browser, exchanges the authorization code, and persists refreshed tokens using the crate’s storage helpers.

## Detailed Behavior
- `perform_oauth_login`:
  1. Binds `127.0.0.1:0`, builds the redirect URI, and spawns a callback server thread (`spawn_callback_server`) that listens for `/callback` requests.
  2. Creates an `OAuthState`, starts authorization with the requested scopes, and obtains an authorization URL.
  3. Prints the URL, attempts `webbrowser::open`, and waits up to 5 minutes for the callback via `tokio::time::timeout` on a oneshot channel.
  4. After receiving `code` and `state`, calls `handle_callback`, retrieves credentials (`get_credentials`), wraps them in `StoredOAuthTokens`, and saves them with `save_oauth_tokens` using the configured store mode.
- `CallbackServerGuard` ensures the tiny_http server unblocks when the function exits.
- `spawn_callback_server` runs in a blocking task, responding to success/failure and sending the parsed code/state to the async channel.
- `parse_oauth_callback` parses query parameters, percent-decodes values, and validates the `/callback` route.

## Broader Context
- Invoked by CLI flows (e.g., TUI environment login) when users authenticate MCP servers. Pairs with `oauth.rs` storage and `auth_status.rs` discovery logic.

## Technical Debt
- None; improvements (e.g., better error messaging) can build on top of this structure.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./oauth.rs.spec.md
  - ./auth_status.rs.spec.md
