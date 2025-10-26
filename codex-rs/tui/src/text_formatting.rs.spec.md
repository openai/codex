## Overview
Collection of text utilities used across the TUI: truncating tool results, compacting JSON for better wrapping, grapheme-aware truncation, and centering/truncating file paths with ellipses.

## Detailed Behavior
- `format_and_truncate_tool_result`:
  - Calculates a grapheme budget (`max_lines * line_width - max_lines`) to account for potential wrapping.
  - Attempts to parse the input as JSON via `format_json_compact`; if successful, truncates the formatted output instead of the raw text.
- `format_json_compact`:
  - Parses JSON into a `serde_json::Value`, pretty-prints it, then strips newlines while inserting spaces after `:` and `,` when not inside strings. This yields single-line JSON with whitespace that Ratatui can wrap.
  - Handles escape sequences and ensures strings retain internal spacing.
- `truncate_text`:
  - Iterates grapheme clusters to avoid slicing inside multi-codepoint characters.
  - If truncation is needed and `max_graphemes ≥ 3`, keeps the first `max_graphemes - 3` graphemes and appends `…`; otherwise trims to the exact length.
- `center_truncate_path`:
  - Splits the path on `MAIN_SEPARATOR`, respecting leading/trailing separators.
  - Generates possible combinations of leading and trailing segments to retain, preferring to keep up to two suffix segments.
  - Inserts a middle ellipsis when segments are omitted, and front-truncates overly long segments using an ellipsis prefix.
  - Falls back to front truncation when nothing fits.
- Tests cover truncation edge cases (emoji, combining marks, zero budgets), JSON formatting variations, and path handling across POSIX/Windows-style separators.

## Broader Context
- Used by the status card, resume picker, and diff renderers to show concise yet informative text within constrained UI widths.
- Ensures JSON tool outputs wrap correctly in terminals without relying on `serde_json::to_string_pretty`, which would consume too many lines.

## Technical Debt
- Grapheme-based truncation still approximates terminal cell widths; double-width characters could still overshoot the intended layout.
- JSON compaction is handcrafted; switching to a dedicated library or token stream could reduce maintenance burden.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Incorporate terminal cell width calculations (e.g., via `unicode_width`) into `format_and_truncate_tool_result` to better respect double-width glyphs.
related_specs:
  - status/helpers.rs.spec.md
  - resume_picker.rs.spec.md
