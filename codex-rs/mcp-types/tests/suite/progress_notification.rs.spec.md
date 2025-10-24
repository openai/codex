## Overview
`mcp-types/tests/suite/progress_notification.rs` confirms that progress notifications emitted by MCP servers deserialize into the generated Rust types and convert through the notification dispatcher.

## Detailed Behavior
- Defines a JSON payload for `notifications/progress` that includes message text, fractional progress, a numeric progress token, and a total.
- Deserializes the payload into `JSONRPCMessage`, extracts the notification variant, and passes it through `ServerNotification::try_from` to exercise the generated method dispatch.
- Asserts that the resulting `ProgressNotificationParams` struct matches the expected values (including conversion of the integer token into `ProgressToken::Integer`).

## Broader Context
- Guards against schema drift affecting progress reporting—a key signal for IDE integrations that surface long-running tool activity. If the schema renames fields or changes types, this test will inform maintainers immediately.
- Context can't yet be determined for richer progress metadata (e.g., stage names). Additional assertions can be added once the schema evolves.

## Technical Debt
- None observed; the test covers the critical notification conversion path.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../../mod.spec.md
  - ../../src/lib.rs.spec.md
  - ./mod.rs.spec.md
