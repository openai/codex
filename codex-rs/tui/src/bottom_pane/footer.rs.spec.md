## Overview
`bottom_pane::footer` renders the dynamic hint footer beneath the composer. It toggles between shortcut summaries, overlays, Ctrl+C reminders, and context window indicators based on application state and user actions.

## Detailed Behavior
- `FooterProps` encapsulates the current mode (`FooterMode`), whether Esc backtrack hints or Shift+Enter hints should show, task-running state, and context window usage.
- Mode helpers:
  - `toggle_shortcut_mode` switches between summary and overlay views.
  - `esc_hint_mode` enables Esc hints when no task is running.
  - `reset_mode_after_activity` returns to the summary mode after user input.
- `footer_lines(props)` assembles the appropriate `Line`s for the active mode:
  - `CtrlCReminder` shows dimmed “Ctrl+C again to interrupt/quit”.
  - `ShortcutSummary` combines context usage and “? for shortcuts”.
  - `ShortcutOverlay` renders multiple shortcut entries via `SHORTCUTS` descriptors and `build_columns`.
  - `EscHint` shows Esc/Esc messaging depending on backtrack state.
  - `ContextOnly` displays the context window percentage.
- Rendering:
  - `footer_height` counts lines for layout.
  - `render_footer` prefixes lines with indentation (`FOOTER_INDENT_COLS`) and writes them via `Paragraph`.
- Helper functions build individual lines (Ctrl+C reminder, Esc hints) and format overlay columns using `key_hint` for consistent key styling.

## Broader Context
- `ChatComposer` calls these helpers to compute footer height and render the footer area as part of the bottom pane layout.

## Technical Debt
- None; logic is modular and ties directly into the shared shortcut descriptor table.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./chat_composer.rs.spec.md
  - ../status_indicator_widget.rs.spec.md
