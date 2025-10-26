## Overview
`popup_consts` centralizes shared constants and hint helpers for bottom-pane popups so command/file selectors use consistent sizing and footer messaging.

## Detailed Behavior
- `MAX_POPUP_ROWS` caps the number of rows rendered by any popup (command list, file search, etc.).
- `standard_popup_hint_line()` returns a `Line` instructing users to press Enter to confirm or Esc to exit, reusing `key_hint::plain` for consistent key styling.

## Broader Context
- Referenced by `command_popup`, `file_search_popup`, and other popup widgets to keep UI behavior aligned.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./command_popup.rs.spec.md
  - ./file_search_popup.rs.spec.md
