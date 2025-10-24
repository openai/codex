## Overview
`core::unified_exec::session` wraps the underlying PTY process (`codex_utils_pty::ExecCommandSession`) and buffers output for unified exec sessions. It enforces byte limits, exposes handles for asynchronous consumers, and detects sandbox denials before responses propagate back to the caller.

## Detailed Behavior
- `OutputBufferState` is a deque-backed ring buffer. `push_chunk` appends bytes, trims from the front once the 1 MiB cap is exceeded, and always keeps `total_bytes` up to date. `drain` hands all chunks to the caller, resetting `total_bytes`, while `snapshot` clones the current window for inspection.
- `UnifiedExecSession::new` wires the PTY broadcast channel into an async task that pushes chunks into the buffer and notifies waiters. The session stores the buffer, notify handle, background task, and sandbox type.
- Accessors (`writer_sender`, `output_handles`, `has_exited`, `exit_code`) let the session manager write input, wait for output, and observe process state without holding locks.
- `check_for_sandbox_denial` blocks briefly for late-arriving output, aggregates the buffered transcript, and runs `is_likely_sandbox_denied`. When the heuristic triggers it truncates the message via `truncate_middle` and returns a `UnifiedExecError::SandboxDenied` carrying the full `ExecToolCallOutput`.
- `from_spawned` adapts `SpawnedPty` into a `UnifiedExecSession`. It checks whether the process has already exited (via `exit_rx`) and immediately runs the sandbox check to surface denials. Long-lived sessions keep watching `exit_rx` so that early termination still runs the denial logic. Dropping the session aborts the background output task to stop leaks.

## Broader Context
- The session object underpins `UnifiedExecSessionManager`, which clones its handles to read buffered output and feed stdin. Tests in `mod.rs` exercise the buffer trimming behaviors provided here.
- Sandbox detection aligns unified exec with the rest of the exec stack: the same `is_likely_sandbox_denied` heuristic powers tooling responses elsewhere, ensuring consistent messaging.
- Context can't yet be determined for multi-stream support (stderr vs stdout); current PTY sessions only capture a single stream, and future improvements may need to propagate combined stderr/stdout semantics.

## Technical Debt
- `check_for_sandbox_denial` synthesizes an empty stderr stream, so denials that only emit stderr risk being misclassified; wiring the PTY’s stderr (if available) would improve accuracy.
- The 20–50 ms waits around exit detection are empirical; exposing configuration or adaptive backoff would avoid missed output under heavy load.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Feed real stderr data into `ExecToolCallOutput` during sandbox checks to enhance denial detection.
    - Revisit the fixed delays (20 ms/50 ms) used when probing exit channels to balance responsiveness and completeness.
related_specs:
  - ./mod.rs.spec.md
  - ./session_manager.rs.spec.md
  - ../exec.rs.spec.md
  - ../tools/runtimes/unified_exec.rs.spec.md
