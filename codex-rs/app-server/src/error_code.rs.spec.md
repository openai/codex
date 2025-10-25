## Overview
`error_code` centralizes the JSON-RPC error codes used by the app server when responding to client requests.

## Detailed Behavior
- Defines two `i64` constants:
  - `INVALID_REQUEST_ERROR_CODE` (`-32600`) for malformed or unsupported requests.
  - `INTERNAL_ERROR_CODE` (`-32603`) for unexpected failures while servicing a request.
- These codes align with the standard JSON-RPC 2.0 specification.

## Broader Context
- Imported throughout `codex_message_processor` and `outgoing_message` to guarantee consistent error reporting across all endpoints.
- Keeps numeric codes in one place so updates can be made without touching every call site.

## Technical Debt
- None; the file intentionally remains a minimal constant holder.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./codex_message_processor.rs.spec.md
  - ./outgoing_message.rs.spec.md
