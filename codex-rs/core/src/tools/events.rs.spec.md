## Overview
`core::tools::events` emits protocol events tied to tool execution. It tracks start/end stages for shell commands and apply_patch runs, integrates diff tracking, and packages exec output summaries for the model and telemetry.

## Detailed Behavior
- `ToolEventCtx` encapsulates the session, turn, call ID, and optional diff tracker. It is passed through event emitters to publish events and update diffs.
- `ToolEmitter` provides concrete emitters for shell, apply_patch, and unified exec tools:
  - `Shell` emits `ExecCommandBegin` and `ExecCommandEnd`, supplying parsed commands, exit codes, durations, and formatted output (using helpers from `tools/mod.rs`).
  - `ApplyPatch` locks the turn diff tracker to snapshot baseline changes, emits `PatchApplyBegin` with change metadata, and later `PatchApplyEnd` based on exec results.
  - `UnifiedExec` currently emits begin events only; TODO notes indicate future success/failure signaling.
- Utility functions (`emit_exec_command_begin`, `emit_exec_end`, `emit_patch_end`) assemble the appropriate `EventMsg` variants, including aggregated output and diff summaries.

## Broader Context
- `handle_container_exec_with_params` and other tool entry points construct emitters to ensure consistent event sequencing regardless of runtime. UI layers rely on these events to display live tool execution status and diffs.
- Integration with `SharedTurnDiffTracker` ensures apply_patch updates contribute to the turn diff summary before completion.
- Context can't yet be determined for unified exec completion events; the TODO hints at pending work to emit end/failure notifications.

## Technical Debt
- Unified exec lacks success/failure emissions (TODO comment). Implementing those events will align telemetry with other tool types.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Implement `ToolEmitter::UnifiedExec` success/failure events so unified exec sessions generate matching completion telemetry.
related_specs:
  - ./mod.rs.spec.md
  - ./context.rs.spec.md
  - ../turn_diff_tracker.rs.spec.md
