## Overview
`codex-tui::markdown` is a thin helper that renders Markdown strings into Ratatui lines and appends them to an existing transcript vector. It wraps the richer renderer (`markdown_render.rs`) and line utilities to keep callers concise.

## Detailed Behavior
- `append_markdown(markdown_source, width, lines)`:
  - Calls `render_markdown_text_with_width` to produce a `Text<'static>` using optional wrapping width.
  - Pushes resulting lines into the provided `Vec<Line<'static>>` via `push_owned_lines`.
- Tests ensure behavior matches expectations (plain text remains single-line, ordered list markers stay intact, indented code preserves whitespace, citations remain plain text).

## Broader Context
- Chat widget and markdown stream collectors use this helper to append agent responses and streaming deltas without managing renderer details.
- Width-aware rendering keeps transcripts consistent across different layout widths.
- Context can't yet be determined for future extensions (e.g., custom styling); this function delegates to the renderer to handle details.

## Technical Debt
- None; the helper is intentionally simple.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./markdown_render.rs.spec.md
  - ./markdown_stream.rs.spec.md
