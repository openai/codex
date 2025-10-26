## Overview
`exec_cell` renders command execution output in the transcript. It tracks command calls, whether they’re exploratory (read/list/search) or active executions, and formats stdout/stderr snippets with metadata such as duration and exit code.

## Detailed Behavior
- Re-exports:
  - Models (`ExecCell`, `ExecCall`, `CommandOutput`) from `model.rs`.
  - Rendering helpers (`new_active_exec_command`, `output_lines`, `spinner`, constants) from `render.rs`.
- `ExecCell` implements `HistoryCell`, providing transcript lines for both exploratory commands and full executions, including highlighted commands, formatted output, and success/failure indicators.

## Broader Context
- `ChatWidget` inserts `ExecCell` instances whenever the agent runs shell or apply-patch commands, displaying ongoing progress and final results in the transcript.

## Technical Debt
- Shared behavior is centralized; no additional debt identified beyond the complexity already documented in `model.rs` and `render.rs`.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./model.rs.spec.md
  - ./render.rs.spec.md
