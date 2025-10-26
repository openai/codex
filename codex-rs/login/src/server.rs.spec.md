## Overview
`server` drives the browser-based `codex login` experience. It launches a lightweight local HTTP server, constructs OAuth URLs with PKCE parameters, exchanges authorization codes for tokens (and API keys), enforces workspace restrictions, and persists credential state.

## Detailed Behavior
- `ServerOptions` captures runtime configuration (Codex home, client ID, issuer URL, port, whether to open a browser, forced state, optional workspace restriction).
- `run_login_server(opts)`:
  - Generates PKCE codes and random state (unless a state is forced, e.g., for tests).
  - Binds a `tiny_http::Server` on the requested port, retrying up to 10 times and attempting `/cancel` against an existing server if the address is in use.
  - Constructs the authorization URL (including `originator`, optional `allowed_workspace_id`) and optionally launches the system browser.
  - Bridges blocking `tiny_http` requests into an async channel handled on a Tokio task that selects between inbound HTTP requests and shutdown notifications.
  - Handles three routes via `process_request`:
    - `/auth/callback`: exchanges the authorization code via `exchange_code_for_tokens`, ensures the workspace matches restrictions, optionally exchanges the ID token for an API key, and persists auth tokens (`persist_tokens_async`). Returns a redirect to `/success` or an error response.
    - `/success`: serves a bundled HTML success page and shuts down with `Ok(())`.
    - `/cancel`: cancels the login flow with an `Interrupted` error.
  - Returns a `LoginServer` handle containing the auth URL, actual port, join handle, and a `ShutdownHandle` for cancellation.
- `LoginServer::block_until_done` awaits the server task and unwraps join errors; `cancel` / `cancel_handle` expose manual shutdown.
- `ShutdownHandle` wraps a `tokio::Notify` to coordinate cancellation between tasks.
- `process_request` ensures state consistency, handles workspace restriction errors through `login_error_response`, and invokes helper functions:
  - `exchange_code_for_tokens` (POST `/oauth/token`) and `obtain_api_key` (token exchange).
  - `ensure_workspace_allowed` to enforce workspace restrictions by inspecting JWT claims.
  - `persist_tokens_async` to write `auth.json` (`CodexAuth`) without blocking the request loop.
- `send_response_with_disconnect` works around `tiny_http`’s keep-alive behavior by writing raw HTTP responses with `Connection: close` to prevent hangs on subsequent logins.
- Additional helpers:
  - `build_authorize_url`, `generate_state`, `send_cancel_request`, `bind_server`.
  - JWT parsing via `jwt_auth_claims` and the redirect-success query composer `compose_success_url`.

## Broader Context
- Invoked by the CLI (and potentially desktop apps) to authenticate users through a browser flow. Shares persistence and workspace safeguards with device-code login to ensure consistent auth state.
- Depends on `codex-core` auth utilities and token parsing logic, maintaining parity with other Codex services.

## Technical Debt
- Heavy reliance on synchronous `tiny_http` with manual workarounds (custom response writer, `thread::spawn` bridge). Migrating to an async HTTP server would simplify cancellation logic and improve maintainability.

---
tech_debt:
  severity: high
  highest_priority_items:
    - Replace `tiny_http` + manual response handling with an async HTTP server to eliminate fragile `Connection: close` hacks and simplify request processing.
related_specs:
  - ../mod.spec.md
  - ./device_code_auth.rs.spec.md
  - ./pkce.rs.spec.md
