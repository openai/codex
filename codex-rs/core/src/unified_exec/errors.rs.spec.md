## Overview
`core::unified_exec::errors` defines `UnifiedExecError`, the error surface shared by unified exec sessions, the tool runtime, and callers consuming the PTY API. Each variant carries enough context to map failures into tool errors or user-facing messages.

## Detailed Behavior
- `CreateSession` wraps string messages from session startup (sandbox orchestration failures, spawn errors) so higher layers can surface actionable details.
- `UnknownSessionId` represents attempts to send input to a session that has already closed or was never established; it carries the session ID for logging/telemetry.
- `WriteToStdin` indicates backpressure or channel closure when forwarding input to the PTY’s stdin.
- `MissingCommandLine` guards against empty `ExecEnv` command arrays while constructing shell invocations.
- `SandboxDenied` stores both a human-readable message and the `ExecToolCallOutput`; tool runtimes convert this into `CodexErr::Sandbox` so the user sees consistent sandbox messaging.
- Convenience constructors (`create_session`, `sandbox_denied`) centralize variant creation.

## Broader Context
- The tool orchestrator (`tools/runtimes/unified_exec.rs`) converts these errors into `ToolError` variants, preserving sandbox payloads and surfacing session lifecycle issues to the UI.
- Session management (`session_manager.rs`) emits these variants as it manipulates PTY state, while `session.rs` triggers `SandboxDenied` when heuristics detect policy failures.

## Technical Debt
- None noted; the enum cleanly maps error cases without leaking implementation details.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.rs.spec.md
  - ./session_manager.rs.spec.md
  - ./session.rs.spec.md
  - ../tools/runtimes/unified_exec.rs.spec.md
