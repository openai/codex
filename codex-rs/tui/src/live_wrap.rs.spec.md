## Overview
Implements a plain-text line wrapper that incrementally converts streaming input into fixed-width rows while tracking whether breaks were explicit (newline) or caused by wrapping. This precursor feeds styled wrapping logic elsewhere in the TUI, ensuring consistent width calculations across Unicode glyphs.

## Detailed Behavior
- `Row` represents each rendered row with its text and a flag indicating explicit newlines.
- `RowBuilder` maintains a `current_line` buffer and already-wrapped `rows` for previous output. Key operations:
  - `new` clamps the requested width to at least one column.
  - `push_fragment` appends text, respecting embedded `\n`; each newline flushes the current line as an explicit row.
  - `wrap_current_line` repeatedly trims prefixes that fit within `target_width` via `take_prefix_by_width`; when the remaining suffix exactly fits, it stays buffered for future fragments.
  - `flush_current_line` finalizes the line, pushing an explicit row (even empty) to preserve layout when a newline lands on a width boundary.
  - `set_width` rewraps all existing rows by reconstructing the source text, simplifying width changes.
  - `display_rows` includes the current partial line, while `drain_commit_ready` evicts the oldest rows beyond a retention limit so callers can stream committed history elsewhere.
- `take_prefix_by_width` walks scalar values, summing `UnicodeWidthChar` values until the limit is reached, and returns a prefix string plus the remaining suffix and consumed width. When no character fits (e.g., width 0) it returns an empty prefix.
- Tests cover ASCII and Unicode width handling, ensure fragments produce deterministic rows regardless of chunking, verify newline semantics, and confirm rewrapping after width changes.

## Broader Context
- The composer and inline history rely on this utility before applying ANSI-aware wrapping, guaranteeing that word wrapping respects emoji/CJK widths just like the streamed output.
- Higher-level wrappers integrate line styles later but reuse the same width calculations to keep layout stable between preview and final render.

## Technical Debt
- `set_width` rewinds and rewraps by concatenating all prior rows, which can be costly for large histories. Incremental reflow could avoid O(n) copies.
- `take_prefix_by_width` ignores grapheme clusters; combining marks could produce visual widths that differ from `UnicodeWidthChar` estimates.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Optimize `set_width` to avoid rebuilding the entire buffer on every size change for large histories.
    - Investigate grapheme-aware width accounting so combining characters wrap correctly.
related_specs:
  - mod.spec.md
  - insert_history.rs.spec.md
