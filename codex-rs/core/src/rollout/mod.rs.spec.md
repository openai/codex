## Overview
`core::rollout` collects the primitives for persisting and retrieving Codex session rollouts. The module exports the rollout recorder, pagination utilities, and policy helpers used to decide which protocol items land in the JSONL transcripts stored under `~/.codex/sessions`.

## Detailed Behavior
- Defines constants that establish directory layout (`sessions/`, `archived_sessions/`) and restrict listing operations to interactive sources (`Cli`, `VSCode`).
- Re-exports `SessionMeta` from `codex-protocol` so callers can reason about session metadata without importing the protocol crate directly.
- Publicly exposes `RolloutRecorder` (write path) and `find_conversation_path_by_id_str` (read path), while keeping `policy` internal to ensure persistence rules stay centralized.
- Gate-keeps tests behind the `tests` submodule so integration cases live next to the implementation.

## Broader Context
- The recorder is wired into `codex.rs` to capture every conversation when rollout recording is enabled; the listing helpers back CLI/TUI diagnostics that surface recent sessions.
- Context can't yet be determined for the archived directory workflow; specs for higher-level management should clarify when files move from `sessions/` to `archived_sessions/`.

## Technical Debt
- The module-level API exports span writer, reader, and policy concerns simultaneously; introducing a façade struct or grouping by responsibility would clarify intended usage patterns.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Investigate whether `archived_sessions` needs dedicated helpers or should be hidden behind a façade alongside the live `sessions` directory.
related_specs:
  - ../lib.rs.spec.md
  - ./list.rs.spec.md
  - ./policy.rs.spec.md
  - ./recorder.rs.spec.md
  - ./tests.rs.spec.md
