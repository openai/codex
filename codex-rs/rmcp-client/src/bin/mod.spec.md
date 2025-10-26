## Overview
The binaries under `rmcp-client/src/bin` provide lightweight MCP test servers that exercise different transport implementations. They are intended for integration tests, local troubleshooting, and protocol experiments—not for production deployment.

## Detailed Behavior
- `rmcp_test_server.rs` starts a stdio-based MCP tool server exposing an `echo` tool. It demonstrates basic tool registration, argument validation, and structured content in responses.
- `test_stdio_server.rs` extends the stdio server with resource enumeration APIs, returning a memo resource/template alongside the `echo` tool so clients can validate resource workflows.
- `test_streamable_http_server.rs` runs the same tool/resource surface behind the streamable HTTP transport (Axum + `StreamableHttpService`), optionally enforcing a bearer token via `MCP_EXPECT_BEARER`.
- All binaries share schema definitions, re-exported helper functions (`stdio()`), and consistent logging so test harnesses can interact with them predictably.

## Broader Context
- Codex integration tests launch these binaries to validate RMCP client behaviors (tool calls, resource reads, HTTP streaming). They keep the examples close to the real server contract without embedding test-only logic inside the library.

## Technical Debt
- Helpers duplicate schema and response setup; consolidating common fixtures would make it easier to evolve the test surface across transports.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Extract shared tool/resource helpers so future protocol changes update all binaries consistently.
related_specs:
  - ../mod.spec.md
