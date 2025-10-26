## Overview
`app-server/tests/common` centralizes fixtures and helpers for Codex app-server integration tests. It spins up mock MCP processes, seeds ChatGPT auth files, and synthesizes streaming model responses to keep suite code tidy.

## Detailed Behavior
- `lib.rs` re-exports fixture builders and helper functions so tests can import from a single module path. It also converts raw JSON-RPC responses into typed protocol structs.
- `auth_fixtures.rs` fabricates ChatGPT auth.json files with customizable tokens, account IDs, and plan metadata.
- `mcp_process.rs` manages a spawned `codex-app-server` child process, providing methods to send JSON-RPC requests, consume notifications, and await responses.
- `mock_model_server.rs` mounts a mock `/v1/chat/completions` SSE endpoint using `wiremock` with deterministic response sequencing.
- `responses.rs` constructs SSE payload strings for shell/apply_patch tool calls and assistant messages.

## Broader Context
- Integration suites under `app-server/tests` import these helpers to simulate Codex client interactions without relying on production infrastructure.
- MCP tooling specs in Phase 2 reference the same protocol types (`codex_app_server_protocol`) that these helpers encode and decode.

## Technical Debt
- MCP process helper emits verbose stderr debugging logs; coordinated logging hooks could reduce noise when tests fail.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Replace ad-hoc stderr forwarding with structured logging so integration test output stays manageable.
related_specs:
  - ./lib.rs.spec.md
  - ./auth_fixtures.rs.spec.md
  - ./mcp_process.rs.spec.md
  - ./mock_model_server.rs.spec.md
  - ./responses.rs.spec.md
