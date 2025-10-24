## Overview
`codex-tui::public_widgets::composer_input` exposes a simplified wrapper around the internal chat composer so other crates can reuse Codex’s multi-line input behavior. It provides submission semantics, paste handling, and hint overrides without pulling in the entire TUI.

## Detailed Behavior
- `ComposerInput`:
  - Internally owns a `ChatComposer`, plus a private `AppEvent` channel to consume widget-generated events (`AppEventSender`).
  - `new` (and `Default`) create a composer with enhanced key support, placeholder text, and paste-burst detection disabled.
- API surface:
  - `is_empty`, `clear` manage text state.
  - `input(KeyEvent)` returns `ComposerAction::Submitted(text)` when the user submits (Enter) or `None` otherwise; drains internal events to keep the channel clean.
  - `handle_paste` forwards pasted text to the composer’s heuristics, returning whether it was handled.
  - `set_hint_items` / `clear_hint_items` override footer hints for caller-specific shortcuts.
  - `desired_height`, `cursor_pos`, `render_ref` delegate to the inner composer for layout/rendering.
  - `is_in_paste_burst`, `flush_paste_burst_if_due`, and `recommended_flush_delay` expose paste-burst timing helpers so callers can schedule redraws during incremental flushes.

## Broader Context
- Used by auxiliary tools (e.g., `codex-cloud-tasks`) to leverage Codex’s polished input experience without re-implementing composer logic.
- Integrates with the same key handling semantics as the main TUI (Shift+Enter for newline).
- Context can't yet be determined for richer customization (themes, validation); the current API focuses on core composer behavior.

## Technical Debt
- None; the wrapper intentionally keeps a narrow surface.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../bottom_pane/mod.rs.spec.md
  - ../app_event.rs.spec.md
