## Overview
`core::exec` executes shell commands for tool calls. It builds sandbox-aware execution environments, launches child processes, streams incremental output, and normalizes results into `ExecToolCallOutput` structures used by higher-level tooling.

## Detailed Behavior
- `ExecParams` encapsulates command arguments, working directory, timeout, environment overrides, optional escalation, justification, and optional `arg0`. `timeout_duration` converts the optional timeout (default 10 s) into a `Duration`.
- `SandboxType` enumerates supported sandboxes (`None`, `MacosSeatbelt`, `LinuxSeccomp`), and `StdoutStream` defines the channel used to emit live `ExecCommandOutputDelta` events.
- `process_exec_tool_call` converts `ExecParams` into a `CommandSpec`, transforms it via `SandboxManager::transform`, and routes execution through `sandboxing::execute_env`. It respects network permissions (`CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR`) and handles both apply_patch and shell commands.
- `execute_exec_env` clones an `ExecEnv`, calls `exec`, and passes the result to `finalize_exec_result`, which:
  - Detects timeouts (per-platform) and returns `SandboxErr::Timeout` with captured output.
  - Calls `is_likely_sandbox_denied` to translate sandbox-related failures into `SandboxErr::Denied`.
  - Returns `ExecToolCallOutput` with UTF‑8-decoded stdout/stderr/aggregated streams, exit code, duration, and timeout flag.
- `exec` spawns the child process via `spawn_child_async`, wiring stdin/stdout/stderr according to `StdioPolicy::RedirectForShellTool`. It then calls `consume_truncated_output` with a timeout and optional stdout stream.
- `consume_truncated_output`:
  - Reads stdout/stderr concurrently with `read_capped`, which emits delta events (capped at 10 000 per call) and accumulates bytes for final aggregation.
  - Uses `tokio::select!` to enforce timeouts (issuing synthetic exit statuses) and handles `ctrl_c` cancellation by sending SIGKILL.
  - Aggregates output chunks into `StreamOutput<Vec<u8>>` and returns `RawExecToolCallOutput`.
- `is_likely_sandbox_denied` heuristically checks exit codes and output text for sandbox-related keywords, accounting for seccomp SIGSYS exits on Linux, to differentiate genuine sandbox denials from ordinary failures.
- Helper functions convert between byte and string `StreamOutput`, append buffers efficiently, and synthesize exit statuses on Unix/Windows.

## Broader Context
- Tool handlers (`shell`, `apply_patch`, `unified_exec`) rely on this module for consistent execution semantics, including sandbox enforcement and live event streaming.
- `SandboxManager` orchestrates sandbox selection and command wrapping; `exec` focuses on process spawning and output capture.
- stdout streaming integrates with the conversation event loop, allowing UIs to display running command output in near-real time while still returning the full aggregated output when execution finishes.

## Technical Debt
- None explicitly noted; heuristics in `is_likely_sandbox_denied` may need refinement as sandbox implementations evolve.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./sandboxing/mod.rs.spec.md
  - ./spawn.rs.spec.md
  - ./tools/mod.rs.spec.md
