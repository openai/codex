## Overview
`exec_cell::render` converts execution data into transcript lines with syntax highlighting, spinners, and trimmed output snippets. It also provides helper functions for building new active exec cells.

## Detailed Behavior
- `new_active_exec_command` creates an `ExecCell` for a newly started command, recording the `call_id`, command args, and parsed metadata.
- `output_lines(output, params)` produces dimmed output lines (stdout/stderr) capped at `TOOL_CALL_MAX_LINES`, optionally showing angle-pipe prefixes and indicating omitted lines with ellipsis.
- `spinner(start_time)` chooses between shimmer animation or blinking glyph based on color support and elapsed time.
- `HistoryCell` implementation:
  - `display_lines`, `transcript_lines`, and `desired_transcript_height` format command lines using bash highlighting (`highlight_bash_to_lines`), wrap content via `RtOptions`, and append result lines with exit code/duration.
  - Exploratory commands render differently (e.g., aggregated file listings).
- `OutputLinesParams` allows callers to request only error output, include prefixes, or show the angle-branch indicator.

## Broader Context
- Combined with `model.rs`, this module powers the transcript view for exec tool calls handled in `ChatWidget`.

## Technical Debt
- Rendering mixes command/body formatting; any future styling overhaul should centralize prefix/indent logic to avoid duplication across history cells.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Extract shared prefix and wrapping logic if additional command-like history cells are introduced.
related_specs:
  - ./model.rs.spec.md
  - ../history_cell.rs.spec.md
