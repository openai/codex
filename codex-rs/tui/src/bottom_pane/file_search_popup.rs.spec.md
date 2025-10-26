## Overview
`file_search_popup` renders the inline file-search selector that appears when the user types a file-prefixed slash command. It tracks asynchronous query results, handles selection movement, and displays matches with fuzzy highlights.

## Detailed Behavior
- `FileSearchPopup` stores the current/expected query (`display_query`, `pending_query`), a `waiting` flag, cached `FileMatch` results, and shared `ScrollState`.
- `set_query(query)` updates the pending query, marks the popup as waiting, and optionally clears matches if the new query doesn’t extend the previous display query.
- `set_empty_prompt()` resets the state for an idle (“@”) prompt, showing helper text until more characters are typed.
- `set_matches(query, matches)` applies new results only if they match the pending query; it updates `display_query`, stores matches, marks `waiting = false`, and adjusts selection visibility.
- `move_up` / `move_down` wrap the selection index and maintain the visible window bounded by `MAX_POPUP_ROWS`.
- `selected_match()` returns the currently highlighted path (if any), allowing the composer to insert it into the slash command.
- `calculate_required_height()` chooses between 1 row (no matches) and the clamped match count, keeping the popup stable while results stream in.
- Implements `WidgetRef` to render rows via `render_rows`, passing an inset area, the current rows, scroll state, and an empty message (`"loading..."` or `"no matches"`).

## Broader Context
- `ChatComposer` drives the popup: it calls `set_query`, `set_matches`, handles key movement, and inserts the selected path when the user accepts a result.

## Technical Debt
- None identified; module cleanly separates rendering from the composer logic.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./chat_composer.rs.spec.md
  - ./selection_popup_common.rs.spec.md
