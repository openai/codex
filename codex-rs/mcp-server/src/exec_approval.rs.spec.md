## Overview
`exec_approval` packages Codex execution approval events into MCP elicitation requests and applies the user’s decision back to the Codex conversation.

## Detailed Behavior
- `ExecApprovalElicitRequestParams` mirrors `mcp_types::ElicitRequestParams`, adding Codex-specific metadata (call IDs, command, cwd, parsed command tokens) so the client can display context alongside the approval prompt.
- `ExecApprovalResponse` carries the selected `ReviewDecision`. A TODO notes that the structure should be upgraded to match the MCP `ElicitResult` schema (`action` + `content`).
- `handle_exec_approval_request`:
  - Formats the shell command using `shlex::try_join` for readability.
  - Serializes the params, returning an MCP `INVALID_PARAMS` error if serialization fails.
  - Sends an `ElicitRequest` via `OutgoingMessageSender::send_request`, then spawns a task to await the response asynchronously.
- `on_exec_approval_response` awaits the oneshot, defaulting to `ReviewDecision::Denied` on decoding errors, and submits the result to the Codex conversation as `Op::ExecApproval`.

## Broader Context
- Invoked from the MCP tool runner (`./codex_tool_runner.rs.spec.md`) whenever a Codex session pauses for shell command approval.
- Works alongside `patch_approval` for apply-patch prompts, sharing the same error code constant exported by the runner.
- Uses `OutgoingMessageSender` (`./outgoing_message.rs.spec.md`) to negotiate with MCP clients and bridges the response back into the Codex protocol.

## Technical Debt
- Response payloads should adopt the MCP `ElicitResult` format; until then, consumers must rely on the simplified `{ decision }` structure.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Update `ExecApprovalResponse` to conform to the MCP `ElicitResult` schema (action/content fields).
related_specs:
  - ./codex_tool_runner.rs.spec.md
  - ./outgoing_message.rs.spec.md
  - ./patch_approval.rs.spec.md
