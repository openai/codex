## Overview
`list_selection_view` implements a generic modal list used for selection dialogs (e.g., workspace pickers, custom prompt browsers). It supports optional search, keyboard navigation, contextual descriptions, and footer hints.

## Detailed Behavior
- `SelectionItem` represents each entry with name, descriptions, actions (`SelectionAction` closures), shortcut hints, and flags for current selection/dismissal behavior.
- `SelectionViewParams` configures the view (title, subtitle, footer hint, items, search behavior, header content).
- `ListSelectionView` wraps items, filtered indices, `ScrollState`, search query, and completion flag. It renders via `Renderable` header columns and `render_rows` for list content.
- Search:
  - `apply_filter` filters items by `search_value` when search is enabled and updates `filtered_indices`.
  - `search_input` handles character insertion and deletion, toggling `is_searchable`.
- Navigation & selection:
  - `move_up`, `move_down` wrap around filtered results, `ensure_visible` keeping selection in view.
  - `handle_key_event` processes Enter, Esc, `Ctrl-C`, search toggles, and arrow navigation; selecting an item triggers its actions (`SelectionAction`) and optionally dismisses the view.
- Rendering:
  - `render` draws header, search bar (when enabled), result list, and footer hint.
  - `calculate_required_height` uses `measure_rows_height` to size the popup respecting `MAX_POPUP_ROWS`.
  - Current items can display additional descriptions when selected.
- Integration:
  - Implements `BottomPaneView`, exposing `desired_height`, `cursor_pos`, `handle_paste` (no-op), `on_ctrl_c`, and `is_complete`.

## Broader Context
- `BottomPane` instantiates `ListSelectionView` for features like session resume selection or slash command helpers, providing a consistent modal experience.

## Technical Debt
- The view mixes searching, rendering, and action dispatch; extracting the search/filter logic or action invocation into helpers could simplify future maintenance.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Untangle search handling and action dispatch from rendering to reduce complexity and ease testing.
related_specs:
  - ./selection_popup_common.rs.spec.md
  - ./chat_composer.rs.spec.md
