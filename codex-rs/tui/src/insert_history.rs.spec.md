## Overview
Provides the low-level routine that inserts wrapped history lines above the inline viewport by emitting ANSI escape sequences directly through the terminal backend. It mirrors `ratatui`’s diffing logic so the scrollback matches what the TUI displayed, including style propagation for blockquotes, lists, and other formatted output.

## Detailed Behavior
- `insert_history_lines` accepts a `custom_terminal::Terminal` and owned `Line` values:
  - Queries the backend for screen size, then wraps lines with `word_wrap_lines_borrowed` so the terminal scrollback matches word-wrapped layout rather than VT100 char wrapping.
  - If the viewport is not anchored to the bottom, adjusts the scroll region to push it downward while preserving other rows.
  - Limits the scroll region to everything above the current viewport, moves the cursor to the insertion point, and iterates wrapped lines. For each line it sets foreground/background colors (respecting line-level style), clears to end of line, merges line style into span styles, and writes spans using `write_spans`.
  - Restores the global scroll region and cursor position, updating the terminal viewport if it scrolled.
- Custom commands:
  - `SetScrollRegion` (`ESC[{start};{end}r`) and `ResetScrollRegion` (`ESC[r]`) constrain scrolling to specific regions so history insertion doesn’t disturb the viewport.
- `write_spans` streams spans to the backend writer, minimizing attribute churn:
  - Tracks current `Modifier`, foreground, and background colors. `ModifierDiff` computes which ANSI attributes to apply/remove.
  - Writes the span content, then resets colors/attributes at the end.
- Tests assert ANSI correctness by driving a `VT100Backend`:
  - Snapshot-style checks ensure bold sequences are emitted correctly, blockquotes keep green styling across wrapped lines, prefixes reset color as expected, and deeply nested markdown retains colored markers.

## Broader Context
- The history pane relies on this helper when pasting previous turns back into scrollback while the inline viewport focuses on the composer.
- Shares modifier-diff logic with the custom terminal (`custom_terminal.rs`) to guarantee consistent ANSI emission between buffered rendering and out-of-band history insertion.
- Markdown rendering (`markdown_render.rs`) produces the `Line` values that flow through this module.

## Technical Debt
- Uses manual ANSI sequences with best-effort `.ok()` error handling, which can mask write failures; propagating errors upward would improve resilience.
- Scroll-region handling assumes VT100-compatible terminals. Windows support for these sequences remains TODO; documenting confirmed compatibility or adding fallbacks would clarify behavior.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Surface write failures instead of ignoring `queue!` errors so terminal issues can bubble up.
    - Audit Windows terminal support for scroll-region commands and provide fallbacks if unsupported.
related_specs:
  - custom_terminal.rs.spec.md
  - markdown_render.rs.spec.md
