## Overview
`chatwidget::interrupts` queues event-driven interruptions (approvals, tool call lifecycle, exec begin/end) that should be processed after the current streaming batch completes. It ensures UI disruptions happen at safe times.

## Detailed Behavior
- `QueuedInterrupt` enumerates deferred actions: exec approvals, apply-patch approvals, exec/mcp begin/end events, and patch completion.
- `InterruptManager` maintains a FIFO queue:
  - `push_*` helpers enqueue each interrupt type.
  - `flush_all(chat)` drains the queue and calls corresponding handler methods on `ChatWidget` (`handle_exec_approval_now`, `handle_mcp_end_now`, etc.).
  - `is_empty` checks whether pending interrupts remain.
- Used by `ChatWidget` to defer interrupts while rendering streaming output, avoiding UI re-entrancy issues during ongoing draws.

## Broader Context
- Keeps approval prompts and status updates synchronized with stream completion without losing events from codex-core.

## Technical Debt
- None; module is small and focused.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../chatwidget.rs.spec.md
