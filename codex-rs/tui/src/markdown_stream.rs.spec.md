## Overview
`codex-tui::markdown_stream` accumulates streaming Markdown (e.g., agent deltas) and emits complete logical lines once newline boundaries are reached. It enables smooth incremental rendering in the transcript while avoiding partial line flicker.

## Detailed Behavior
- `MarkdownStreamCollector` state:
  - `buffer`: accumulated raw text.
  - `committed_line_count`: number of rendered lines already emitted.
  - Optional `width` for wrapping (passed to `markdown::append_markdown`).
- Methods:
  - `new(width)`: initialize the collector.
  - `clear()`: reset buffer and commit count.
  - `push_delta(delta)`: append incremental text (e.g., SSE deltas).
  - `commit_complete_lines()`: render the buffer, identify the last newline, and return only the newly completed lines since the last commit. Incomplete trailing lines remain buffered.
  - `finalize_and_drain()`: flush remaining content (appending a newline if necessary), return uncommitted lines, and reset state.
- Utility `simulate_stream_markdown_for_tests` helps unit tests feed deltas and validate output.
- Tests ensure commit gating works, finalize flushes partial lines, and Markdown styling (e.g., block quotes, nested lists) is preserved during streaming.

## Broader Context
- Chat widget uses this collector to stream model output into history cells, only appending fully-rendered lines to avoid flicker and maintain consistent styling.
- Works alongside `markdown_render` and `wrapping` to ensure streamed content respects formatting.
- Context can't yet be determined for multi-paragraph streaming; the collector currently commits once newline boundaries are seen.

## Technical Debt
- None significant; the collector is focused and well tested.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./markdown.rs.spec.md
  - ./markdown_render.rs.spec.md
