## Overview
`codex-app-server::message_processor` routes JSON-RPC requests/responses between the client and Codex core. It enforces initialization semantics and delegates conversation handling to `CodexMessageProcessor`.

## Detailed Behavior
- `MessageProcessor::new`:
  - Wraps the outgoing sender with `Arc`.
  - Creates a shared `AuthManager` and `ConversationManager` (SessionSource::VSCode).
  - Constructs `CodexMessageProcessor`, passing auth manager, conversation manager, outgoing sender, optional sandbox executable, and shared config.
  - Maintains an `initialized` flag to enforce JSON-RPC `initialize` handshake.
- Request handling (`process_request`):
  - Converts raw `JSONRPCRequest` into `ClientRequest` using serde.
  - Special-cases `ClientRequest::Initialize`:
    - Returns error if already initialized.
    - Otherwise updates `USER_AGENT_SUFFIX`, sends `InitializeResponse` (with user agent from `get_codex_user_agent`), and marks `initialized = true`.
  - For other requests, returns error if `initialized` is false.
  - Defer to `CodexMessageProcessor::process_request` for actual operations (search, open file, etc.).
- Notifications/responses:
  - `process_notification` logs incoming notifications (none expected currently).
  - `process_response` relays responses to clients via `OutgoingMessageSender::notify_client_response`.
  - `process_error` logs errors received from the peer.

## Broader Context
- `run_main` (lib.rs) invokes `MessageProcessor` on a dedicated task. `CodexMessageProcessor` handles domain-specific commands (fuzzy search, message routing), while `MessageProcessor` stays focused on JSON-RPC handshake and message type dispatch.
- Clients (VSCode extension, other frontends) interact via this JSON-RPC interface over stdio.

## Technical Debt
- None noted; initialization guard keeps state consistent.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./lib.rs.spec.md
  - ./codex_message_processor.rs.spec.md (to add)
