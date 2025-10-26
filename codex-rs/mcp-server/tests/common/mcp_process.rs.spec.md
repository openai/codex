## Overview
`mcp_server::tests::common::mcp_process` spins up the `codex-mcp-server` binary and provides JSON-RPC helpers to drive tool calls, initialization, and legacy notifications during integration tests.

## Detailed Behavior
- `McpProcess::new`/`new_with_env` spawn the server with piped stdio, configurable environment overrides, and stderr forwarding to the test runner.
- Maintains an `AtomicI64` counter for JSON-RPC request IDs and holds onto stdin/stdout handles for message exchange.
- Includes helpers to:
  - Run the MCP initialization handshake (`initialize`).
  - Send tool call requests (`send_call_tool_request`), respond to server prompts, and post notifications back (`notify`).
  - Await specific message types (`read_stream_until_request_message`, `read_stream_until_response_message`, `read_stream_until_legacy_task_complete_notification`) while enforcing that unexpected messages cause hard failures.
- Serializes messages using `serde_json` and leverages `mcp_types` enums for protocol structures, ensuring fixture traffic matches production expectations.

## Broader Context
- Used alongside WireMock streaming fixtures to validate Codex MCP server behavior end to end, complementing the Phase 2 MCP specs.

## Technical Debt
- Similar to the app-server helper, the file is large and intertwines spawning, logging, and protocol utilities; extracting reusable JSON-RPC utilities would reduce duplication.

---
tech_debt:
  severity: high
  highest_priority_items:
    - Factor shared JSON-RPC harness logic into reusable modules to avoid drift between app-server and mcp-server test fixtures.
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
