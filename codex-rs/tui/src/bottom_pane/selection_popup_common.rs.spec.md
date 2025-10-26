## Overview
`selection_popup_common` contains shared rendering utilities for popup lists (command palette, file search). It converts data rows into Ratatui lines, handles fuzzy highlight styling, and enforces consistent alignment between item names and descriptions.

## Detailed Behavior
- `GenericDisplayRow` describes a popup entry, including name, optional shortcut, match highlight indices, “current” flag, and description text.
- `compute_desc_col` inspects visible rows to determine the description column offset (max name width + padding), clamping to the available width to avoid overflow.
- `build_full_line` constructs a `Line` for a row:
  - Applies fuzzy-match bolding to characters at `match_indices`.
  - Truncates names with ellipsis when they exceed the allotted width.
  - Inserts display shortcuts and pads space before appending a dimmed description.
- `render_rows(area, buf, rows, state, max_results, empty_message)`:
  - Determines which rows to render based on selection and `ScrollState`.
  - Calls `build_full_line`, wraps descriptions with aligned subsequent indent using `wrapping::word_wrap_line`, and renders each wrapped line.
  - Applies cyan bold styling to the selected row and shows a dimmed `empty_message` when no rows are present.

## Broader Context
- Popups share these helpers to keep layout and styling consistent, reducing duplication across `command_popup` and `file_search_popup`.

## Technical Debt
- None; module is narrowly focused on presentation logic.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./command_popup.rs.spec.md
  - ./file_search_popup.rs.spec.md
