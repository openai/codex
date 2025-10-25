## Overview
`OutgoingMessageSender` abstracts JSON-RPC responses, server-initiated requests, and notifications sent from the app server to the client. It multiplexes callbacks so higher-level handlers can await responses without managing channel plumbing.

## Detailed Behavior
- Maintains an atomic counter for generated request IDs and a `HashMap<RequestId, oneshot::Sender<Result>>` guarded by a mutex to resolve in-flight approvals.
- `send_request`:
  - Allocates a new integer request ID, stores the sender in the callback map, and emits an `OutgoingMessage::Request` down the `mpsc::UnboundedSender`.
  - Returns a `oneshot::Receiver<Result>`; callers await it to obtain the eventual JSON-RPC result.
- `notify_client_response`:
  - Removes the callback and fulfills the oneshot with the received `Result`, logging a warning if no callback is found or the channel is closed.
- `send_response` serializes arbitrary response payloads; on serialization failure it falls back to `send_error` with `INTERNAL_ERROR_CODE`.
- `send_server_notification` emits strongly-typed notifications via the `AppServerNotification` variant. `send_notification` remains for legacy untyped notifications with a TODO to migrate to `ServerNotification`.
- `send_error` packages JSON-RPC errors for the client.
- Serialization helpers:
  - `OutgoingMessage` is an untagged enum covering requests, responses, errors, legacy notifications, and app-server notifications.
  - `OutgoingNotification`, `OutgoingResponse`, and `OutgoingError` are simple data carriers used by the sender.
- Tests assert JSON serialization for structured notifications (e.g., login completion, rate-limit updates) to prevent regressions when protocol definitions change.

## Broader Context
- Consumed by `CodexMessageProcessor` (`./codex_message_processor.rs.spec.md`) and the outer `MessageProcessor` to send responses, raise approval prompts, and publish account state updates.
- Bridges internal approval flows with the client UI: `send_request` is the mechanism used for patch/exec approvals and login handshakes.

## Technical Debt
- Legacy `send_notification` usage should be retired in favor of typed `ServerNotification`s, as noted in the comment.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Migrate remaining callers from `send_notification` to `send_server_notification`.
related_specs:
  - ./codex_message_processor.rs.spec.md
  - ./message_processor.rs.spec.md
  - ./error_code.rs.spec.md
