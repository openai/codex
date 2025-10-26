## Overview
`codex_utils_pty::lib` provides asynchronous PTY spawning and session management. It wires `portable_pty` child processes into Tokio channels so Codex can stream shell outputs, send user input, and monitor exit status without blocking.

## Detailed Behavior
- `ExecCommandSession` holds the channel senders/receivers, child killer handle, join handles for reader, writer, and wait tasks, and shared exit tracking state. It offers:
  - `writer_sender` for pushing input bytes into the PTY.
  - `output_receiver` to subscribe to broadcast stdout chunks.
  - `has_exited`/`exit_code` for status polling backed by atomic + mutex state.
  - A `Drop` impl that kills the child and aborts background tasks to prevent resource leaks.
- `SpawnedPty` packages an `ExecCommandSession`, a broadcast receiver for initial output subscription, and a one-shot exit receiver.
- `spawn_pty_process(program, args, cwd, env)`:
  - Validates the program name, opens a 24x80 PTY via `portable_pty::native_pty_system`, and seeds the command with a clean environment populated from the provided map.
  - Sets up independent reader and writer tasks:
    - Reader uses `spawn_blocking` to pull from the PTY master, translating EOF/interruption into loop termination and broadcasting each chunk (`Vec<u8>`) to subscribers.
    - Writer runs on Tokio, receiving bytes from an mpsc channel, writing and flushing them through an `Arc<TokioMutex<_>>` guard.
  - Spawns a blocking waiter that records the exit code, marks `exit_status`, updates a shared `Option<i32>`, and fulfills the one-shot channel.
  - Returns the assembled `SpawnedPty` so callers can integrate the session with their async event loops.
- Channels are sized (writer 128, broadcast 256) to balance responsiveness with back-pressure, while reader backoff uses short sleeps to recover from `WouldBlock`.

## Broader Context
- Unified exec tooling in `codex-core` wraps `ExecCommandSession` to enforce sandbox detection and byte limits before relaying output back to the model (`core/src/unified_exec/session.rs` and `session_manager.rs`).
- Future integrations may reuse the same primitives for interactive debugging workflows or custom tooling once PTY sizing becomes configurable.

## Technical Debt
- PTY size is hard-coded to 24x80, which may truncate wide outputs. Resizing support (via `portable_pty::PtySize`) and surfacing window adjustments to callers would improve fidelity.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Add configurable PTY sizing (and optional resize hooks) so callers can match user terminal dimensions.
related_specs:
  - ../mod.spec.md
  - ../../core/src/unified_exec/session.rs.spec.md
  - ../../core/src/unified_exec/session_manager.rs.spec.md
