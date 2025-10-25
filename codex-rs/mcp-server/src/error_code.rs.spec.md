## Overview
`error_code` centralizes the JSON-RPC error codes used by the MCP server when responding to client requests.

## Detailed Behavior
- Declares `INVALID_REQUEST_ERROR_CODE` (`-32600`) for malformed or unsupported payloads.
- Declares `INTERNAL_ERROR_CODE` (`-32603`) for unexpected server failures.
- Both constants follow the JSON-RPC 2.0 spec and keep the numeric values consistent across modules.

## Broader Context
- Imported by `outgoing_message`, `exec_approval`, `patch_approval`, and the message processor to maintain uniform error semantics.

## Technical Debt
- None; the module intentionally remains minimal.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./outgoing_message.rs.spec.md
  - ./exec_approval.rs.spec.md
  - ./patch_approval.rs.spec.md
