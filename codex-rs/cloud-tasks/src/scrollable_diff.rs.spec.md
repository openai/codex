## Overview
`scrollable_diff.rs` provides a lightweight scroll-and-wrap component for rendering diffs and long text inside the TUI. It tracks viewport geometry, wrapped lines, and scroll position, mirroring ratatui’s expectations without pulling in heavier widgets.

## Detailed Behavior
- `ScrollViewState` stores the scroll offset, viewport height, and content height, with a `clamp` helper to keep scroll within bounds.
- `ScrollableDiff` owns:
  - Raw lines, cached wrapped lines, and a mapping from wrapped lines to original indices.
  - Optional wrap width (`wrap_cols`) to trigger rewrapping when the terminal width changes.
  - A public `state` field for direct scroll inspection.
- API highlights:
  - `set_content` replaces raw lines and clears caches.
  - `set_width` rewraps lines when width changes (`rewrap` handles Unicode width, replaces tabs with spaces, and splits on whitespace/punctuation).
  - `set_viewport` updates height and clamps scroll.
  - `wrapped_lines`/`wrapped_src_indices` expose cached lines for rendering; `raw_line_at` fetches original lines.
  - `scroll_by`, `page_by`, `to_top`, `to_bottom`, and `percent_scrolled` drive navigation.
- `rewrap` iterates over characters, maintaining soft-break positions to avoid splitting words and handling multi-width Unicode code points via `unicode_width`.

## Broader Context
- Used by `app::DiffOverlay` and rendered in `ui.rs` when displaying diffs or assistant output. Keeps diff rendering independent of the ratatui List/Paragraph widgets for fine-grained control.

## Technical Debt
- None noted; the component already guards against zero-width viewports and supports percent scrolled reporting for status bars.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./app.rs.spec.md
  - ./ui.rs.spec.md
