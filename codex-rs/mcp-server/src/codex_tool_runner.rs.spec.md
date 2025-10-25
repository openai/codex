## Overview
`codex_tool_runner` bridges MCP `tools/call` requests to Codex conversations. It starts or resumes Codex sessions, forwards every emitted event back to the MCP client, and returns a `CallToolResult` when the turn finishes or errors.

## Detailed Behavior
- Exposes `INVALID_PARAMS_ERROR_CODE` for shared error handling with approval helpers.
- `run_codex_tool_session`:
  - Creates a fresh conversation via `ConversationManager::new_conversation`, returning an MCP error result if startup fails.
  - Emits a synthesized `SessionConfigured` event to the client so tooling can bootstrap UI state.
  - Uses the MCP request ID as the submission `sub_id`, records the mapping in `running_requests_id_to_codex_uuid`, and submits the initial prompt as `Op::UserInput`.
  - Delegates to `run_codex_tool_session_inner` to stream events.
- `run_codex_tool_session_reply` resumes an existing conversation, reuses the stored `Arc<CodexConversation>`, and re-enters the same inner loop.
- `run_codex_tool_session_inner`:
  - Repeatedly awaits `codex.next_event()`, forwarding every event through `OutgoingMessageSender::send_event_as_notification` with metadata linking back to the originating request.
  - Handles key event types:
    - `ExecApprovalRequest` → `handle_exec_approval_request` to solicit MCP approval while preserving call IDs and parsed commands.
    - `ApplyPatchApprovalRequest` → `handle_patch_approval_request`.
    - `TaskComplete` → returns a successful `CallToolResult` (with optional agent message text), removes the running request mapping, and terminates.
    - `Error` → returns an error result describing the failure.
  - For unhandled deltas (agent reasoning, message deltas, etc.), it logs TODOs while still broadcasting the raw notifications.
  - If `next_event` returns an error, responds with a `CallToolResult` flagged as `is_error = true`.
- `run_codex_tool_session_reply` ensures replies include the ongoing conversation ID in the tracking map so future events can be correlated.

## Broader Context
- Called by the MCP `MessageProcessor` when servicing `codex` and `codex-reply` tool invocations (`./message_processor.rs.spec.md`).
- Works with `exec_approval` and `patch_approval` to translate Codex approval events into MCP elicitation requests.
- Uses `OutgoingMessageSender` (`./outgoing_message.rs.spec.md`) to emit events, notifications, and final results.

## Technical Debt
- Several event types (`AgentMessageDelta`, `AgentReasoningDelta`, etc.) are currently ignored beyond passive notification; the TODOs note the need for richer MCP representations in future updates.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Map agent reasoning / delta events to structured MCP responses instead of dropping them.
related_specs:
  - ./message_processor.rs.spec.md
  - ./outgoing_message.rs.spec.md
  - ./exec_approval.rs.spec.md
  - ./patch_approval.rs.spec.md
  - ./codex_tool_config.rs.spec.md
