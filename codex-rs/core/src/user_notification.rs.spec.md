## Overview
`core::user_notification` implements the fire-and-forget hook that runs a user-specified command after a turn completes. It serializes notification payloads to JSON and invokes the configured program without blocking the agent loop.

## Detailed Behavior
- `UserNotifier` stores an optional notification command (`Vec<String>`). `notify` guards against empty commands before delegating to `invoke_notify`.
- `invoke_notify` serializes the `UserNotification` enum to JSON and spawns the configured command, passing the JSON blob as the final argument. Errors in serialization or process spawning are logged with `tracing::error`/`warn` but do not bubble up.
- `UserNotification::AgentTurnComplete` captures:
  - `thread_id`, `turn_id`, and working directory.
  - User input messages that opened the turn.
  - The most recent assistant response (when available).
- Tests validate JSON serialization so downstream automation knows the exact schema it will receive.

## Broader Context
- CLI/TUI surfaces allow users to register notification commands (e.g., desktop alerts). Keeping the payload consistent ensures external scripts can parse events without coupling to internal types.
- This module pairs with `review_format` and `project_doc` to supply contextual data as part of the notification pipeline.
- Context can't yet be determined for future notification types (failures, approvals); the tagged enum design allows additional variants without breaking existing listeners.

## Technical Debt
- The spawn is fire-and-forget; lacking timeout or output capture makes debugging notification failures harder. Offering optional logging/exit status checks would aid troubleshooting.
- Command configuration is unvalidated; adding early checks (existence, executability) could warn users before notifications silently fail.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Provide optional diagnostics (e.g., capturing stderr or exit status) so users can debug notification command failures.
    - Validate notification command configuration during startup to surface missing binaries before turns begin.
related_specs:
  - ./config.rs.spec.md
  - ./review_format.rs.spec.md
  - ./user_instructions.rs.spec.md
