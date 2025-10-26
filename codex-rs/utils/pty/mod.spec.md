## Overview
`codex-utils-pty` wraps `portable_pty` and Tokio primitives to launch interactive processes behind pseudo terminals, exposing handles for streaming output, writing input, and retrieving exit status.

## Detailed Behavior
- Re-exports session management types from `src/lib.rs`, notably `ExecCommandSession`, `SpawnedPty`, and `spawn_pty_process`.
- Depends on `portable_pty` for OS-specific PTY handling but keeps higher-level coordination in this crate so callers only need a single import.

## Broader Context
- Consumed by `codex-core` unified exec sessions to back shell tool calls with interactive PTYs while maintaining async-friendly channels.
- Context can't yet be determined for other potential consumers; revisit if additional crates adopt the session helpers.

## Technical Debt
- PTY window size defaults to `24x80` in the current implementation; future consumers may require negotiation or resizing support.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Allow callers to configure PTY window size or negotiate it dynamically to avoid truncated output on wide terminals.
related_specs:
  - ./src/lib.rs.spec.md
