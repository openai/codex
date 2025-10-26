## Overview
`mcp-server/tests/common` bundles helpers used by Codex MCP server integration tests. It launches the server binary under test, constructs mock SSE responses, and simplifies JSON-RPC decoding.

## Detailed Behavior
- `lib.rs` exposes `McpProcess`, SSE builders, and a convenience `to_response` function mirroring the app-server test utilities.
- `mcp_process.rs` manages a spawned `codex-mcp-server`, handling JSON-RPC message I/O and providing helpers for initialization, tool invocation, and legacy notification detection.
- `mock_model_server.rs` and `responses.rs` replicate the SSE streaming helpers used for app-server tests, enabling deterministic tool call simulations.

## Broader Context
- Complements the Phase 2 MCP server specs by giving tests a controlled harness for exercising MCP protocol flows end to end.
- Shared SSE builders keep parity with the app-server test suite to reduce duplication.

## Technical Debt
- Process helper still prints verbose debug output to stderr; structured logging would keep failing tests easier to read.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Convert raw stderr forwarding to structured logging hooks so noisy test output can be filtered.
related_specs:
  - ./lib.rs.spec.md
  - ./mcp_process.rs.spec.md
  - ./mock_model_server.rs.spec.md
  - ./responses.rs.spec.md
