## Overview
`codex-rmcp-client` wraps the official RMCP Rust SDK so Codex can talk to Model Context Protocol servers. It provides an `RmcpClient` with stdio and streamable HTTP transports, OAuth login orchestration, credential storage, and helper utilities for environment detection and logging.

## Detailed Behavior
- `src/lib.rs` re-exports the primary surface: `RmcpClient`, OAuth helpers, auth-status checks, and credential-management APIs.
- `src/rmcp_client.rs` implements the client wrapper, handling transport setup, initialization, tool/resource operations, and OAuth token persistence.
- `src/oauth.rs`, `src/perform_oauth_login.rs`, and `src/auth_status.rs` manage credential storage, login flows, and discovery of supported auth mechanisms.
- `src/utils.rs`, `src/logging_client_handler.rs`, and `src/find_codex_home.rs` provide supporting utilities (environment setup, logging adapters, configuration paths).

## Broader Context
- Used by the Codex core tool router to connect to MCP tools, supporting both local child processes and remote HTTP endpoints with OAuth.

## Technical Debt
- None beyond TODOs already noted in source (e.g., better credential store placement).

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/rmcp_client.rs.spec.md
  - ./src/oauth.rs.spec.md
  - ./src/perform_oauth_login.rs.spec.md
  - ./src/auth_status.rs.spec.md
  - ./src/logging_client_handler.rs.spec.md
  - ./src/utils.rs.spec.md
  - ./src/find_codex_home.rs.spec.md
