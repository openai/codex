## Overview
`rmcp_client.rs` wraps the RMCP Rust SDK to provide Codex’s MCP client. It supports stdio-based servers and streamable HTTP endpoints (with optional OAuth), tracks connection state, and exposes high-level methods that mirror the MCP protocol (`initialize`, `list_tools`, `call_tool`, etc.).

## Detailed Behavior
- Connection management:
  - `PendingTransport` captures the transport variant (child process, HTTP, HTTP with OAuth runtime).
  - `ClientState` toggles between `Connecting` and `Ready`, storing the active `RunningService` and optional `OAuthPersistor`.
- Constructors:
  - `new_stdio_client` spawns a subprocess with sanitized environment (`create_env_for_mcp_server`), pipes stdin/stdout, and logs stderr asynchronously.
  - `new_streamable_http_client` builds HTTP transports, reading stored OAuth tokens when available or using bearer tokens; creates `OAuthPersistor` when OAuth is in play.
- `initialize` converts `mcp_types` params to `rmcp` models, starts the service, performs the handshake with optional timeout, and updates state to `Ready`. It persists refreshed OAuth tokens after initialization.
- MCP operations (`list_tools`, `list_resources`, `list_resource_templates`, `read_resource`, `call_tool`) all:
  - Acquire the service via `self.service()`.
  - Convert requests with `convert_to_rmcp`, run with `run_with_timeout`, convert results back via `convert_to_mcp`, and persist OAuth tokens.
- `persist_oauth_tokens` invokes `OAuthPersistor::persist_if_needed` after each call to ensure refreshed tokens hit disk/keyring.
- `create_oauth_transport_and_runtime` builds an `AuthClient` + `StreamableHttpClientTransport`, seeds it with cached tokens, and returns an `OAuthPersistor` configured with the chosen credential store.

## Broader Context
- Used by Codex tooling to interact with MCP servers in both local (stdio) and remote (streamable HTTP) modes. Integrates with the rest of the crate’s modules for credential storage, logging, and header management.

## Technical Debt
- None beyond inline TODOs (e.g., future elicitation support handled elsewhere).

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./utils.rs.spec.md
  - ./oauth.rs.spec.md
  - ./logging_client_handler.rs.spec.md
