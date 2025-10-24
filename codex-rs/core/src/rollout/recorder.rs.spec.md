## Overview
`core::rollout::recorder` manages the write-path for rollout transcripts. It creates session-specific JSONL files, filters events through the persistence policy, and streams writes on a background task so agent turns never block on disk IO.

## Detailed Behavior
- `RolloutRecorder` owns an mpsc channel to a writer task and exposes:
  - `new` / `RolloutRecorderParams`: create a fresh rollout (`Create`) or resume an existing file (`Resume`). Creation resolves a timestamped path under `~/.codex/sessions/YYYY/MM/DD`, captures `SessionMeta`, and embeds build metadata (CLI version, originator, cwd, initial instructions, `SessionSource`). Resume opens an existing file in append mode without rewriting metadata.
  - `record_items` filters each `RolloutItem` via `policy::is_persisted_response_item` and enqueues the survivors for async writing.
  - `flush` synchronously waits for the writer to finish all queued writes (via oneshot ack) so callers can durably persist before shutdown.
  - `get_rollout_history` rehydrates a rollout file, rebuilding `InitialHistory::Resumed` with parsed items while recovering the canonical `ConversationId`.
  - `shutdown` requests writer termination and waits for acknowledgement.
- Writer pipeline:
  - `rollout_writer` consumes commands, flushing on demand and emitting JSONL lines with UTC timestamps. When a session starts it augments the `SessionMeta` record with Git info collected via `collect_git_info`.
  - `JsonlWriter::write_rollout_item` serializes `RolloutLine` structs and forces an fsync (`flush`) after every line to maintain append durability.
- Helpers like `create_log_file` produce deterministic filenames and ensure directory hierarchies exist; `LogFileInfo` returns the open file and conversation metadata for the caller.

## Broader Context
- The recorder integrates with the conversation pipeline in `codex.rs`, allowing playback, auditing, and tooling like `codex rollout` to inspect agent behavior.
- Filtering aligns with `policy.rs`, so the writer enforces the same rules regardless of where items originate. The listing module (`list.rs`) later reads these files using the same structural assumptions.
- Context can't yet be determined for cross-process coordination (e.g., multiple recorders touching the same file); current design assumes a single writer per session.

## Technical Debt
- Every JSONL write calls `flush`, which prioritizes durability but can degrade throughput on slow disks; batching followed by periodic fsync would improve performance without sacrificing safety if coordinated with `flush`.
- Resume mode trusts existing files without validation; detecting schema drift or partial writes would prevent malformed histories from propagating.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Introduce configurable fsync batching to balance durability and throughput for high-frequency rollouts.
    - Add validation when resuming rollouts to catch truncated or schema-incompatible files before appending.
related_specs:
  - ./mod.rs.spec.md
  - ./policy.rs.spec.md
  - ./list.rs.spec.md
  - ../default_client.rs.spec.md
  - ../git_info.rs.spec.md
