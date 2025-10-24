## Overview
`core::unified_exec::session_manager` drives the lifecycle of unified exec PTY sessions. It mediates between the tool orchestrator (approvals and sandbox retries), the PTY driver, and Codex sessions, providing high-level helpers for launching shells and streaming additional stdin while respecting output and time budgets.

## Detailed Behavior
- `UnifiedExecSessionManager::exec_command` converts the request into a full command line (`shell [-l]c`) and calls `open_session_with_sandbox`. After spawning the PTY it drains output until the yield deadline, truncates to the caller’s token limit, emits a `UnifiedExecResponse`, and, if the process is still alive, stores the session for reuse.
- `write_stdin` looks up an existing session, forwards user input via an `mpsc::Sender`, waits briefly to give the child time to react, and then collects output up to the deadline. Session state is refreshed: exited sessions are removed, running sessions keep their ID.
- `open_session_with_sandbox` hands control to `ToolOrchestrator`, using `UnifiedExecRuntime` to request approvals, choose a sandbox, and run retries when necessary. Errors bubble back as `UnifiedExecError`.
- `open_session_with_exec_env` executes the final `ExecEnv` via `codex_utils_pty::spawn_pty_process`, wrapping the result in `UnifiedExecSession::from_spawned` so sandbox denials can be detected.
- Helper routines manage session state and output collection:
  - `store_session` assigns incremental IDs and tracks sessions in a `Mutex<HashMap<…>>`.
  - `prepare_session_handles` clones the output buffer/notify pair and stdin writer.
  - `collect_output_until_deadline` drains buffered chunks, awaiting `Notify` signals or the deadline with `tokio::select!`.
  - `resolve_max_tokens`, `clamp_yield_time`, and `generate_chunk_id` come from `mod.rs` and are shared across command and stdin flows.

## Broader Context
- This manager is consumed by `tools::runtimes::unified_exec`, which plugs into the tool orchestrator; CLI/TUI frontends ultimately surface the `UnifiedExecResponse` stream to users.
- Sessions are keyed off `Session`/`TurnContext` pairs from `codex.rs`, ensuring approvals and sandbox decisions inherit the ambient turn settings.
- Output buffering relies on `UnifiedExecSession` (see `session.rs`) to normalize PTY streams and detect sandbox denials before responses are sent.

## Technical Debt
- The hard-coded `tokio::time::sleep(100ms)` after writes adds latency and may miss bursts; revisiting the scheduling model (e.g., notifications from the PTY reader) would tighten responsiveness.
- Sequential locking around `sessions` can become a contention point under many concurrent shells; exploring sharded maps or `DashMap` could improve scalability.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Replace the fixed 100 ms post-write sleep with event-driven wakeups to lower latency.
    - Benchmark session map contention; adopt lighter-weight synchronization if unified exec becomes heavily concurrent.
related_specs:
  - ./mod.rs.spec.md
  - ./session.rs.spec.md
  - ./errors.rs.spec.md
  - ../tools/orchestrator.rs.spec.md
  - ../tools/runtimes/unified_exec.rs.spec.md
  - ../sandboxing.rs.spec.md
