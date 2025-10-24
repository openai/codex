## Overview
`core::state::service` groups the heavyweight services shared across a session. `SessionServices` bundles handles to MCP connections, execution managers, telemetry, notifications, and rollout logging so `Session` can pass them into tasks without cloning large structures repeatedly.

## Detailed Behavior
- Fields include:
  - `mcp_connection_manager`: manages active MCP server clients.
  - `unified_exec_manager`: tracks shell/exec processes under the unified execution pipeline.
  - `notifier`: dispatches user-facing notifications (CLI/TUI hooks).
  - `rollout`: `Mutex<Option<RolloutRecorder>>`, allowing tasks to append rollout entries while lazily handling recorder teardown.
  - `user_shell`: cached default shell metadata discovered during session startup.
  - `show_raw_agent_reasoning`: flag controlling whether raw reasoning events are emitted.
  - `auth_manager`: shared pointer to authentication state for MCP and backend interactions.
  - `otel_event_manager`: streams telemetry about session and task lifecycle.
  - `tool_approvals`: `Mutex<ApprovalStore>` storing tool approval decisions for reuse within the session.

## Broader Context
- `SessionServices` is constructed in `Session::new` and cloned as needed (fields like MCP manager implement `Clone` where necessary). Tasks receive references to access tool execution, telemetry, or rollout logging.
- The struct acts as dependency injection for runtime components; specs for tooling (`tools/*`) and telemetry (`otel_init`) should reference the relevant fields they rely on.
- Context can't yet be determined for service granularity; if new domains (e.g., analytics) are added, extending this struct keeps dependencies centralized.

## Technical Debt
- Rollout recorder storage wrapped in `Option` can hide misconfiguration if it becomes `None` unexpectedly; adding instrumentation or a typed state wrapper could improve resilience.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Consider wrapping `rollout` in an enum documenting when the recorder is absent to avoid silently skipping persistence.
related_specs:
  - ./mod.rs.spec.md
  - ../codex.rs.spec.md
  - ../tools/sandboxing/mod.spec.md
