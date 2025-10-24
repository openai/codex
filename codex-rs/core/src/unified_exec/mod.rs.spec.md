## Overview
`core::unified_exec` provides the runtime for interactive PTY execution. The module coordinates approval-flow orchestration, sandbox-aware session startup, streaming output buffers, and follow-up writes to established sessions. It exposes request/response types that higher layers use to run shell commands under Codex’s policy engine.

## Detailed Behavior
- `UnifiedExecSessionManager` is the entry point for `exec_command` (create PTY, gather initial output, optionally retain a session) and `write_stdin` (send more input, poll for output). Both paths clamp yield-time windows and cap token counts using the shared helpers in this module.
- Helpers:
  - `clamp_yield_time` constrains user-provided wait times to the configured `MIN_YIELD_TIME_MS`–`MAX_YIELD_TIME_MS` range, defaulting to `DEFAULT_YIELD_TIME_MS`.
  - `resolve_max_tokens` applies `DEFAULT_MAX_OUTPUT_TOKENS` when callers omit a limit.
  - `generate_chunk_id` produces six hex nibbles so the client can deduplicate streamed updates.
  - `truncate_output_to_tokens` enforces token budgets by counting Unicode scalars, using symmetric truncation when possible and recording the original size for telemetry.
- `UnifiedExecContext`, `ExecCommandRequest`, and `WriteStdinRequest` carry the session, turn metadata, and knobs required by the manager. `UnifiedExecResponse` wraps timing, output, exit status, and optional session identifiers for the caller.
- Constants such as `UNIFIED_EXEC_OUTPUT_MAX_BYTES` keep the buffered transcript within 1 MiB, protecting memory usage when sessions persist across multiple turns.
- The embedded test suite validates buffer trimming, session persistence, multi-session handling, timeouts, approvals, and error mapping (unknown session IDs, completion cleanup).

## Broader Context
- `codex.rs` and the tool runtime (`tools/runtimes/unified_exec.rs`) delegate to `UnifiedExecSessionManager` so the approval/sandbox orchestration remains consistent with non-interactive exec flows.
- Session objects ultimately consume PTY data via `codex_utils_pty`; sandbox policies are applied upstream via `ToolOrchestrator` but errors surface here as `UnifiedExecError`.
- Context can't yet be determined for cross-crate consumers beyond the core agent; future integrations (e.g., IDE plugins) may reuse these response structures directly and should honor the chunk/token semantics captured here.

## Technical Debt
- `generate_chunk_id` relies on a short pseudo-random suffix; longer identifiers or collision checks would harden streaming updates in long-lived sessions.
- Token truncation counts Unicode scalars, not tokenizer tokens; switching to a real token estimator would keep truncation thresholds aligned with model limits.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Replace the six-nibble `generate_chunk_id` with a collision-resistant identifier (e.g., 128-bit).
    - Integrate a tokenizer-aware limit for `truncate_output_to_tokens` to avoid overrun when wide Unicode characters are present.
related_specs:
  - ../lib.rs.spec.md
  - ../exec.rs.spec.md
  - ../tools/runtimes/unified_exec.rs.spec.md
  - ./session_manager.rs.spec.md
  - ./session.rs.spec.md
  - ./errors.rs.spec.md
