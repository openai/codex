## Overview
`app_backtrack` extends `App` with the logic that powers backtracking and transcript overlays. It tracks state transitions when the user presses `Esc`, manages overlay previews, and issues Codex operations to fork or rewind conversations.

## Detailed Behavior
- `BacktrackState` stores runtime flags: whether backtrack is primed, the base conversation ID, which user message is selected, overlay preview state, and pending fork requests.
- `App::handle_backtrack_overlay_event` routes events while the transcript overlay is active:
  - When previewing, `Esc` steps backward, `Enter` confirms, and other events forward to the overlay widget.
  - Outside preview mode, the first `Esc` enables preview and selects the latest user message.
- `handle_backtrack_esc_key` interprets global `Esc` presses (when the composer is empty), priming backtrack, opening the overlay, or stepping through messages depending on current state.
- `request_backtrack` enqueues a `codex_core::protocol::Op::GetPath` request when the user confirms, capturing the base session, message index, and text prefill.
- Overlay helpers (`open_transcript_overlay`, `close_transcript_overlay`, `render_transcript_once`) manage alternate-screen rendering and history flushing back into scrollback.
- Selection utilities (`prime_backtrack`, `open_backtrack_preview`, `begin_overlay_backtrack_preview`, `step_backtrack_and_highlight`, `apply_backtrack_selection`) compute which user turn to highlight based on the transcript and update Ratatui widgets accordingly.
- Additional functions update history cells, fork request metadata, and handle incoming `ConversationPathResponseEvent`s by populating `CompositeHistoryCell` instances.

## Broader Context
- Integrates with `App`’s transcript overlay (`pager_overlay::Overlay`) and history cells, enabling the TUI to preview prior user turns and branch conversations without leaving the terminal.

## Technical Debt
- Backtrack logic spans multiple helper functions and relies on shared mutable state; refactoring toward a dedicated state machine could simplify reasoning about Esc handling and overlay lifecycle.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Extract a focused backtrack state machine to clarify transitions (primed → overlay → confirm) and reduce coupling with the overlay rendering code.
related_specs:
  - ./app.rs.spec.md
  - ./pager_overlay.rs.spec.md
