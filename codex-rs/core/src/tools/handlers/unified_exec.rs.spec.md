## Overview
`core::tools::handlers::unified_exec` bridges the unified exec tool suite (`exec_command`, `write_stdin`) with the session manager. It validates arguments, emits start events, runs the PTY manager, and returns structured JSON describing the most recent output chunk.

## Detailed Behavior
- Accepts `ToolPayload::Function` or `ToolPayload::UnifiedExec`, allowing both function-calling and Responses API pathways to reuse the same handler. Other payload types cause a model-facing error.
- `exec_command` path:
  - Parses `ExecCommandArgs` (command string, shell, login flag, optional yield timeout and token limit).
  - Emits a `ToolEmitter::unified_exec` begin event so clients know the PTY is starting.
  - Invokes `UnifiedExecSessionManager::exec_command`, passing the command, shell, login mode, and streaming configuration via `UnifiedExecContext`.
  - Errors are reported back to the model with descriptive messages.
- `write_stdin` path:
  - Parses `WriteStdinArgs` (session ID, characters, and optional yield/token limits) and calls `UnifiedExecSessionManager::write_stdin`.
- Successful responses are serialized via `SerializedUnifiedExecResponse`, including chunk ID, wall time, output text, optional session ID or exit code, and original token metadata. The JSON string is returned as `ToolOutput::Function { success: Some(true) }`.

## Broader Context
- Unified exec sessions allow shell interaction across multiple tool calls. This handler works alongside the unified exec runtime (`runtimes/unified_exec.rs`) and orchestrator, which manage approvals and sandboxing.
- Event emission currently covers only the start stage (TODO in `events.rs` for completion), so clients rely on manager responses for progress updates.
- Context can't yet be determined for streaming multiple chunks via this handler; responses reflect the latest chunk at invocation time.

## Technical Debt
- Completion/failure events for unified exec are missing (tracked in `tools/events.rs`), so telemetry/shell status is less rich than other tools.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Emit unified exec success/failure events to align with other tool telemetry.
related_specs:
  - ../mod.rs.spec.md
  - ../spec.rs.spec.md
  - ../../events.rs.spec.md
  - ../../../unified_exec/mod.rs.spec.md
