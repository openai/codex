## Overview
`core::tools::runtimes::unified_exec` manages PTY-based unified exec sessions. It requests approvals, prepares sandboxed environments, and delegates to `UnifiedExecSessionManager` to launch or resume sessions.

## Detailed Behavior
- `UnifiedExecRequest` captures the command, working directory, and environment for the PTY session.
- `UnifiedExecRuntime` stores a reference to the session manager and implements `Sandboxable` (auto preference, escalation allowed) and `Approvable` with approval keys comprising the command and working directory.
- `start_approval_async` leverages `with_cached_approval` to reuse session approvals, delegating to `Session::request_command_approval` with optional retry reasons.
- `ToolRuntime::run` builds a `CommandSpec` using `build_command_spec`, transforms it through the current `SandboxAttempt` to obtain an `ExecEnv`, and calls `UnifiedExecSessionManager::open_session_with_exec_env`. Errors are mapped:
  - Sandbox denials from unified exec become `ToolError::Codex(CodexErr::Sandbox(...))`.
  - Other manager errors surface as `ToolError::Rejected` with descriptive messages.

## Broader Context
- Unified exec sessions support long-running PTYs; tooling such as `exec_command` and `write_stdin` rely on this runtime to set up the initial session safely.
- The orchestrator handles approvals and sandbox retries; this runtime focuses on connecting sandboxed environments to the PTY manager.
- Context can't yet be determined for multi-session orchestration; currently, the runtime handles a single session per request, though the manager tracks multiple sessions internally.

## Technical Debt
- None observed; the runtime properly maps manager errors and reuses shared helpers.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.rs.spec.md
  - ../sandboxing.rs.spec.md
  - ../../unified_exec/mod.rs.spec.md
