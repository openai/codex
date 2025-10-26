## Overview
`command_popup` renders the slash-command palette that appears when the composer input starts with `/`. It combines built-in slash commands with saved custom prompts, supports fuzzy filtering, and displays multiline descriptions.

## Detailed Behavior
- `CommandItem` represents either a built-in `SlashCommand` or a user-defined prompt (by index).
- `CommandPopup` loads built-ins (`built_in_slash_commands`) and merges them with custom prompts, excluding name collisions and sorting prompts by name.
- Filtering:
  - `on_composer_text_change` extracts the first token after `/` on the first line, updates `command_filter`, and clamps the scroll selection.
  - `filtered()` performs fuzzy matching against built-in command names and custom prompt entries (`prompts:name`), capturing highlight indices and scores, then sorts results by score/name.
- Rendering:
  - `rows_from_matches` maps matches into `GenericDisplayRow` values, adjusting highlight indices (+1 to account for `/` prefix).
  - `calculate_required_height(width)` asks `measure_rows_height` to compute the wrapped popup height capped by `MAX_POPUP_ROWS`.
  - Implements `WidgetRef` to render rows using `render_rows`, applying insets and scroll position.
- Interaction:
  - `move_up`/`move_down` wrap the selection cursor and ensure it stays within the visible window.
  - `select_current` returns the active `CommandItem`, while `prompt(idx)` exposes the underlying prompt for insertion/submission logic.
  - `selected_row` and `filtered_items` provide metadata used by the composer for hint updates.

## Broader Context
- `ChatComposer` owns a `CommandPopup`, updating it as the user types and retrieving selected items to insert or submit commands.

## Technical Debt
- None; logic remains focused on filtering/presentation and integrates cleanly with shared popup utilities (`selection_popup_common`).

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./chat_composer.rs.spec.md
  - ./selection_popup_common.rs.spec.md
