## Overview
`core::tasks::compact` wraps the auto-compaction workflow as a `SessionTask`. When spawned, it triggers `codex::compact::run_compact_task` to summarize history and shrink the conversation footprint so future turns fit within the model’s context window.

## Detailed Behavior
- `CompactTask` implements `SessionTask` with `kind()` returning `TaskKind::Compact`.
- `run` clones the `Session`, delegates to `compact::run_compact_task`, and discards the returned value (`None`) because compaction emits its own events and does not produce a final assistant message.
- The `CancellationToken` is unused—compaction relies on the underlying `run_compact_task` to honor session cancellation via its own session references.

## Broader Context
- Tasks are triggered by the auto-compaction scheduler or explicit user commands. They persist compacted summaries and notify clients via events defined in `codex::compact`.
- Because compaction can modify history, it is crucial that only one task run at a time; the task runner infrastructure (`Session::spawn_task`) ensures this by aborting existing tasks before starting compaction.
- Context can't yet be determined for partial compaction or chunked summaries; future enhancements may extend `run_compact_task` to return richer metadata.

## Technical Debt
- None observed; the module simply bridges task scheduling with the compaction logic.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.rs.spec.md
  - ../codex/compact.rs.spec.md
