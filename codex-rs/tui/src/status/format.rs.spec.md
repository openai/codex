## Overview
Formatting helpers for status cards. Provides an aligned key/value formatter plus utilities for label deduplication, measuring rendered width, and truncating lines to fit residual space.

## Detailed Behavior
- `FieldFormatter`:
  - `from_labels` inspects all labels to determine the maximum label width, sets a single-space indent, and calculates `value_offset` (`indent + label + colon + 3 spaces`). It also precomputes `value_indent` used for continuation lines.
  - `line` builds a `Line` from a label and value spans via `full_spans`.
  - `continuation` emits a continuation line prefixed with the value indentation (dimmed) so multi-line values line up.
  - `value_width` returns available columns for the value section after subtracting the computed offset.
  - `full_spans` prepends a formatted, dim label (`" label:   "`) to arbitrary value spans.
- Label helpers:
  - `label_span` constructs the dimmed prefix with proper padding so values align under each other.
  - `push_label` adds unique labels to a vector while tracking them in a `BTreeSet`.
- Line utilities:
  - `line_display_width` computes the UTF-8 width (using `UnicodeWidthStr`) of a `Line`.
  - `truncate_line_to_width` trims spans so the line does not exceed `max_width`, iterating characters with `UnicodeWidthChar` to avoid breaking glyphs mid-codepoint.

## Broader Context
- Used by `status/card.rs` to align `/status` output fields and truncate them for the bordered history cell.
- Other status widgets can reuse `FieldFormatter` to maintain the same label/value alignment style.

## Technical Debt
- Formatter assumes monospaced terminal fonts. If the UI expands to proportional fonts, spacing logic would need revisiting.
- Truncation drops trailing spans once the first overflow occurs; adding ellipsis support might improve readability for long values.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Consider adding ellipsis or continuation markers when truncating long values to signal omitted text.
related_specs:
  - card.rs.spec.md
  - helpers.rs.spec.md
