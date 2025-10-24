## Overview
`core::function_tool` defines `FunctionCallError`, the error type propagated when handling model tool calls. It classifies recoverable vs. fatal failures so the router and task loop can decide whether to return output to the model or abort execution.

## Detailed Behavior
- `FunctionCallError::RespondToModel` indicates the error should be sent back to the model as a `FunctionCallOutput` (typically for command failures, sandbox denials, or validation errors).
- `FunctionCallError::Denied` (currently unused) represents explicit user denial; callers treat it similarly to `RespondToModel` but may include stricter messaging.
- `FunctionCallError::MissingLocalShellCallId` surfaces malformed local shell events lacking a call identifier.
- `FunctionCallError::Fatal` signals unrecoverable issues within Codex; the calling loop surfaces the message and aborts the turn.

## Broader Context
- The tool router converts these errors into protocol responses or `CodexErr::Fatal` values, ensuring consistent handling across runtimes and orchestrators.
- Tool runtimes and handlers should choose the appropriate variant so the orchestrator knows when to retry, re-prompt, or stop execution.
- Context can't yet be determined for structured error data; future enhancements might wrap richer diagnostic info instead of plain strings.

## Technical Debt
- The `Denied` variant is flagged with a TODO indicating it should be either used or removed in a follow-up cleanup.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Remove or exercise the `Denied` variant to avoid unused-code suppression.
related_specs:
  - ./tools/router.rs.spec.md
  - ./tools/registry.rs.spec.md
