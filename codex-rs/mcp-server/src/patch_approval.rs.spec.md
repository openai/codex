## Overview
`patch_approval` handles apply-patch approval events for MCP clients. It presents the proposed file changes via an elicitation request and submits the resulting decision back to the Codex conversation.

## Detailed Behavior
- `PatchApprovalElicitRequestParams` mirrors the MCP elicitation schema, augmenting it with Codex metadata (call/event IDs, reason text, grant root, and raw file changes). Optional fields are omitted when absent to keep payloads compact.
- `PatchApprovalResponse` wraps the `ReviewDecision` returned by the client.
- `handle_patch_approval_request`:
  - Builds a human-readable message (including the optional reason) and serializes the params, emitting an MCP `INVALID_PARAMS` error if serialization fails.
  - Sends an `ElicitRequest` through `OutgoingMessageSender::send_request` and spawns an async listener for the response.
- `on_patch_approval_response`:
  - Awaits the oneshot, defaulting to `Denied` on channel errors or JSON decode failures.
  - Submits the decision as `Op::PatchApproval`; on transport errors it retries with a conservative denial.

## Broader Context
- Called by `codex_tool_runner` when Codex proposes filesystem edits during an MCP tool call.
- Complements `exec_approval` and shares the same error handling infrastructure.
- Relies on `OutgoingMessageSender` (`./outgoing_message.rs.spec.md`) to exchange requests/responses with the MCP client.

## Technical Debt
- None beyond the shared need to adopt MCP’s richer `ElicitResult` schema once the ecosystem standardizes it.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./codex_tool_runner.rs.spec.md
  - ./exec_approval.rs.spec.md
  - ./outgoing_message.rs.spec.md
