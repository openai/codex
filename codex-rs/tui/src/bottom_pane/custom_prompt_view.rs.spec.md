## Overview
`custom_prompt_view` displays a modal textarea for entering ad-hoc review instructions or prompts. It wraps the shared `TextArea`, handles submission/cancellation, and renders title/context hints alongside a standard footer.

## Detailed Behavior
- `CustomPromptView::new` accepts title, placeholder, optional context label, and a submission callback.
- Implements `BottomPaneView`:
  - `handle_key_event` handles `Esc` (cancel), `Enter` without modifiers (submit if non-empty), and delegates other keys to the textarea.
  - `handle_paste` inserts pasted text.
  - `cursor_pos` returns the textarea cursor, accounting for title/context rows.
  - `is_complete` signals dismissal after submission or cancellation.
- Rendering:
  - `Renderable::render` draws the title, optional context label, textarea gutter, and placeholder; uses `standard_popup_hint_line` for footer hints.
  - `desired_height` and `input_height` compute required rows based on textarea content and placeholder.

## Broader Context
- `BottomPane` shows `CustomPromptView` when slash commands like `/review` need free-form input before sending instructions to Codex.

## Technical Debt
- None noted; the view is self-contained and relies on `TextArea` for editing semantics.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./chat_composer.rs.spec.md
  - ./bottom_pane_view.rs.spec.md
