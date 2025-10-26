## Overview
`scroll_state` provides reusable selection and scrolling logic for popup-style lists. It tracks the selected index and the topmost visible row, supporting wrap-around navigation and ensuring the selection stays on screen.

## Detailed Behavior
- `ScrollState` holds `selected_idx` (`Option<usize>`) and `scroll_top`.
- Methods:
  - `new`, `reset` initialize state.
  - `clamp_selection(len)` bounds the selection to the range `[0, len-1]`, clearing selection when the list is empty.
  - `move_up_wrap(len)` / `move_down_wrap(len)` adjust the selection with wrap-around semantics.
  - `ensure_visible(len, visible_rows)` adjusts `scroll_top` so the selected row remains within the visible window.

## Broader Context
- Used by `command_popup`, `file_search_popup`, and other selection widgets to share consistent navigation behavior.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./command_popup.rs.spec.md
  - ./file_search_popup.rs.spec.md
