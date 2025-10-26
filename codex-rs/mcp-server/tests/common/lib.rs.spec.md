## Overview
`mcp_server::tests::common::lib` re-exports fixture helpers for integration tests and provides a typed JSON-RPC response conversion helper.

## Detailed Behavior
- Declares submodules (`mcp_process`, `mock_model_server`, `responses`) and publicly re-exports:
  - `McpProcess` for interacting with the server under test.
  - SSE helper constructors for shell/apply_patch tool calls and final assistant messages.
  - `create_mock_chat_completions_server` for deterministic streaming responses.
- `to_response` mirrors the app-server utility by extracting and deserializing the `result` field from a `mcp_types::JSONRPCResponse`.

## Broader Context
- Integration suites import these re-exports to configure tests with minimal boilerplate while keeping fixtures consistent across app and MCP servers.

## Technical Debt
- `to_response` provides limited error context when deserialization fails; augmenting errors with method names would aid debugging.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./mcp_process.rs.spec.md
  - ./responses.rs.spec.md
