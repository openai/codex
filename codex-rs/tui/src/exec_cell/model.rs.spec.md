## Overview
`exec_cell::model` tracks the lifecycle of command executions rendered in the transcript. It records shell commands, their parsed structure, start times, outputs, and durations.

## Detailed Behavior
- `CommandOutput` stores exit code, stdout/stderr strings, and preformatted output (e.g., syntax highlighted diff).
- `ExecCall` represents a single execution attempt:
  - `command`, parsed `ParsedCommand`s, optional output, start time, and duration.
- `ExecCell` aggregates one or more `ExecCall`s:
  - `new` creates a cell for an active call.
  - `with_added_call` appends exploratory commands to an existing cell when the agent is still “exploring” (read/list/search commands).
  - `complete_call` records output/duration for a finished call.
  - `mark_failed` fills in error outputs for calls that did not complete cleanly.
  - Helpers determine whether the cell is exploratory, currently active, or ready to flush to history.
  - `iter_calls` is used by rendering to format each call in order.
- Exploratory detection ensures read-only commands group together, while non-exploring commands flush as soon as their outputs are ready.

## Broader Context
- Rendering logic in `render.rs` uses `ExecCell` data to display command snippets, outputs, and status indicators in the transcript.

## Technical Debt
- None; data model is clearly separated from rendering.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./render.rs.spec.md
