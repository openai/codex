## Overview
`codex-tui::wrapping` provides text-wrapping utilities tailored for Ratatui `Line`s. It wraps both plain strings and styled lines, preserving indentation, markers, and styles—essential for rendering Markdown and history cells within constrained widths.

## Detailed Behavior
- `wrap_ranges` / `wrap_ranges_trim`:
  - Invoke `textwrap::wrap` to produce byte ranges for each wrapped line.
  - The non-trim variant retains trailing whitespace and a sentinel byte (used for cases where spacing matters).
  - The trim variant strips trailing whitespace, suitable for general layouts.
- `RtOptions`:
  - Builder-style struct mirroring `textwrap::Options` but with Ratatui-specific fields (initial/subsequent indent as `Line<'a>`, line ending, algorithms, break words, separators, splitters).
  - Provides default configuration (`OptimalFit` algorithm with high overflow penalty) and fluent setters.
- `word_wrap_line`:
  - Flattens a `Line` into text, tracks span boundaries/styles, and wraps content according to `RtOptions`.
  - Applies initial/subsequent indents, reconstructs Ratatui lines with original styles, and avoids splitting indent markers.
- Helpers (`push_owned_lines`, `line_utils` references) append wrapped lines to existing vectors while maintaining ownership.

## Broader Context
- Markdown renderer (`markdown_render.rs`) uses `word_wrap_line` to wrap paragraphs without losing inline styling or indent markers.
- History cells and diff summaries rely on these utilities to fit content within available column widths while preserving formatting.
- Context can't yet be determined for exotic wrapping (e.g., bidirectional text); the current implementation targets standard LTR content.

## Technical Debt
- Wrapping logic is complex; additional tests for edge cases (multi-codepoint graphemes, combining marks) would strengthen correctness.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Expand test coverage for Unicode edge cases to ensure wrapping remains accurate across languages.
related_specs:
  - ./markdown_render.rs.spec.md
  - ./render/line_utils.rs.spec.md (future)
