## Overview
`codex-tui::app` owns the interactive application state. `App::run` wires together the conversation manager, chat widget, file search, overlays, and event loops for both Codex events and terminal input. It delivers `AppExitInfo` so the binary can print token usage, resume hints, and pending updates.

## Detailed Behavior
- `AppExitInfo` records total token usage, optional conversation ID, and an optional `UpdateAction` (reused by the CLI to trigger upgrades).
- `App` fields:
  - Core services: `ConversationManager`, `AppEventSender`, `ChatWidget`, `AuthManager`, `Config`, active profile, `FileSearchManager`.
  - UI state: `HistoryCell` list, overlays (`Overlay`), deferred transcript lines, enhanced-keys support, commit animation state, backtrack state, feedback collector, and pending update actions.
- `App::run`:
  - Creates an unbounded app event channel (`AppEventSender`) and `ConversationManager` bound to `SessionSource::Cli`.
  - Builds the initial `ChatWidget` based on `ResumeSelection` (start fresh or resume from rollout). Resuming deserializes a saved session via `ConversationManager::resume_conversation_from_rollout`.
  - Instantiates supporting components (file search manager, optional update notification in release builds).
  - Event loop:
    - Uses `tokio::select!` to merge `AppEvent`s and `TuiEvent`s (from `tui.event_stream()`).
    - Schedules initial frame refresh with `frame_requester().schedule_frame()`.
    - Dispatches inbound events to `handle_event` or `handle_tui_event` (both async methods defined later in `app.rs`) to update state, redraw widgets, or submit Codex ops.
  - On loop exit, clears the terminal, logs session end, and returns `AppExitInfo` with token usage, conversation ID, and any pending update action.
- Additional logic inside `app.rs` (not fully shown above) includes:
  - `handle_event`: processes `AppEvent` variants (e.g., new Codex events, file search results, commit animation ticks, approval prompts).
  - `handle_tui_event`: handles keyboard/paste/draw events, implementing navigation, composer input, and UI updates.
  - Helper methods for token usage (`token_usage`), overlay management, backtracking, etc.

## Broader Context
- `App::run` is called by `run_ratatui_app` after onboarding and configuration. Chat widgets, bottom pane, overlays, and renderers reside in other modules referenced here.
- `AppEventSender` and `AppEvent` (see their specs) define the messaging layer between background tasks (file search, commit animation) and the main app loop.
- Context can't yet be determined for future multi-window support; current design assumes a single main event loop targeting a single terminal.

## Technical Debt
- `App` aggregates many responsibilities (network requests, UI flows, background animation); additional structure (e.g., sub-state structs per feature) would reduce complexity.
- Resume logic is intertwined with widget creation; refactoring into a dedicated resume manager could clarify responsibilities.

---
tech_debt:
  severity: high
  highest_priority_items:
    - Decompose `App::handle_event` / `handle_tui_event` into feature-specific handlers (chat, approvals, overlays) to improve maintainability.
    - Extract resume/session restoration into a dedicated component for clearer separation of concerns.
related_specs:
  - ./app_event.rs.spec.md
  - ./app_event_sender.rs.spec.md
  - ./chatwidget.rs.spec.md
  - ./tui.rs.spec.md
