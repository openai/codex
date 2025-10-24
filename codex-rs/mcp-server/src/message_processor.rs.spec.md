## Overview
`codex-mcp-server::message_processor` handles MCP JSON-RPC messages for the prototype MCP server. It enforces initialization, routes tool calls (exec, patch approvals), and delegates codex-specific logic to supporting modules.

## Detailed Behavior
- `MessageProcessor::new`:
  - Wraps the outgoing sender (`OutgoingMessageSender`) in an `Arc`.
  - Loads shared `Config`, instantiates MPC tool runner infrastructure (`codex_tool_runner`, `exec_approval`, `patch_approval`).
  - Maintains an `initialized` flag.
- Request Handling (`process_request`):
  - Parses `JSONRPCRequest` into `ClientRequest`.
  - Special-cases `Initialize` requests:
    - Returns error if already initialized.
    - Otherwise sends an initialization response (with tool metadata, capabilities) and marks initialized.
  - For all other requests, rejects them if initialization hasn’t occurred.
  - Dispatches to tool-specific handlers (`CodexToolRunner`, approvals) to execute commands, run patch approvals, or read resources.
- Notifications/Responses:
  - `process_notification`: logs incoming notifications for debugging (none expected).
  - `process_response`: forwards upstream responses to the client to complete pending tool calls.
  - `process_error`: logs errors reported by the client.
- Error handling uses MCP-specific error codes (`error_code` module) when invalid requests arrive.

## Broader Context
- `run_main` (lib.rs) reads/writes JSON-RPC over stdio. This processor sits between the transport and tool runners, enforcing handshake semantics and relaying requests to Codex tools.
- The MCP server exposes Codex functionality (exec, patch approvals) to MCP-compatible clients (e.g., IDE integrations).

## Technical Debt
- Similar structure to the app server’s message processor; future refactoring might consolidate shared logic (initialize guard, logging) between the servers.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Abstract common JSON-RPC initialization logic if the app server and MCP server continue to diverge minimally.
related_specs:
  - ./lib.rs.spec.md
  - ./codex_tool_runner.rs.spec.md (future)
