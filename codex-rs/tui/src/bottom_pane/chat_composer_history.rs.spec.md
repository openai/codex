## Overview
`chat_composer_history` manages shell-style history navigation (Up/Down) inside the composer. It unifies persistent history fetched from codex-core with entries created during the current session.

## Detailed Behavior
- Tracks metadata (`history_log_id`, `history_entry_count`), local submissions, cached fetched entries, current cursor position, and the last text recalled.
- `record_local_submission` stores new inputs while avoiding duplicates.
- `should_handle_navigation(text, cursor)` determines whether arrow keys should trigger history browsing (e.g., only when cursor is at start and text matches the last recalled entry).
- `navigate_up` / `navigate_down` adjust the history cursor and either return local entries or request missing persistent entries via `AppEvent::CodexOp(Op::GetHistoryEntryRequest)`.
- `on_entry_response` caches fetched entries and returns the text if it matches the current cursor.
- Internal helper `populate_history_at_index` decides whether to return a cached entry or trigger an async fetch.

## Broader Context
- `ChatComposer` instantiates `ChatComposerHistory` to support command-recall semantics similar to shell environments.

## Technical Debt
- None; the state machine cleanly separates history logic from rendering and input handling.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./chat_composer.rs.spec.md
