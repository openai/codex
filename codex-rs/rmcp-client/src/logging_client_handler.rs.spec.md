## Overview
`logging_client_handler.rs` provides a `ClientHandler` implementation for the RMCP SDK that logs server notifications and declines unsupported elicitations. It keeps clients informed about server-side events while integrating with `tracing`.

## Detailed Behavior
- `LoggingClientHandler` wraps the client info advertised during initialization.
- `ClientHandler` implementation:
  - `create_elicitation` logs and declines all elicitations (feature placeholder).
  - Notification hooks log cancellations, progress updates, resource changes, tool/prompt list changes.
  - `on_logging_message` routes server log messages to the appropriate `tracing` level based on `LoggingLevel`.
- `get_info` returns the stored `ClientInfo` so the RMCP service can advertise client identity.

## Broader Context
- Used by `RmcpClient::initialize` to monitor RMCP server behavior without requiring consumers to implement their own handler.

## Technical Debt
- TODO note references future elicitation support (tracked elsewhere).

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./rmcp_client.rs.spec.md
