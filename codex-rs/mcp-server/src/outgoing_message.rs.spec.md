## Overview
`OutgoingMessageSender` for the MCP server encapsulates all outbound JSON-RPC traffic. It generates request IDs, dispatches notifications, and converts Codex events into MCP notifications while tracking callbacks for pending elicitation responses.

## Detailed Behavior
- State:
  - `next_request_id` supplies monotonically increasing integer IDs.
  - `request_id_to_callback` stores oneshot senders for in-flight requests (`ElicitRequest`, etc.).
- Core methods:
  - `send_request` builds an `OutgoingRequest` with the method name and optional parameters, sends it over an `mpsc::UnboundedSender`, and returns a `oneshot::Receiver<Result>` for the response.
  - `notify_client_response` resolves the stored sender or logs a warning if the request ID is unknown/closed.
  - `send_response` serializes arbitrary payloads into a JSON value; falls back to `send_error` with `INTERNAL_ERROR_CODE` when serialization fails.
  - `send_event_as_notification` packages a `codex_core::protocol::Event` into the `codex/event` notification format, injecting optional `_meta.requestId` data.
  - `send_notification` and `send_error` wrap generic notifications and JSON-RPC errors, respectively.
- Serialization layer:
  - `OutgoingMessage` enumerates requests, notifications, responses, and errors. A `From<OutgoingMessage> for JSONRPCMessage` implementation stamps the required `jsonrpc` version and constructs `JSONRPCRequest/Response/Error` structures.
  - `OutgoingNotificationParams` flattens the event payload while stashing MCP metadata under `_meta`.
- Tests ensure `send_event_as_notification` emits the expected JSON payload with and without metadata.

## Broader Context
- Used by the MCP `MessageProcessor` and tool runner to communicate with MCP-compatible clients (`./message_processor.rs.spec.md`, `./codex_tool_runner.rs.spec.md`).
- Shares the callback orchestration model with the app server’s sender but tailors the wire format to MCP (e.g., `codex/event` notifications).

## Technical Debt
- Legacy `send_notification` calls should eventually funnel through higher-level helpers once all MCP notifications standardize on typed flows.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Consolidate direct `send_notification` usages behind structured helpers when the MCP protocol stabilizes.
related_specs:
  - ./message_processor.rs.spec.md
  - ./codex_tool_runner.rs.spec.md
  - ./exec_approval.rs.spec.md
  - ./patch_approval.rs.spec.md
