## Overview
`chatwidget` orchestrates the main chat experience in the TUI. It listens to Codex events, manages history cells, coordinates streaming output, handles approvals, rate-limit warnings, ghost commits, and integrates with the bottom pane for user input.

## Detailed Behavior
- `ChatWidgetInit` captures initialization parameters (config, initial prompt/images, auth manager, feedback sink).
- `ChatWidget` holds:
  - App wiring (`app_event_tx`, `codex_op_tx`, `FrameRequester`).
  - UI components (`BottomPane`, `SessionHeader`, active `HistoryCell`).
  - Conversation and streaming state (`ConversationId`, `StreamController`, queued user messages).
  - Rate limit tracking, status headers, task flags, interrupt manager, ghost commit snapshots, and feedback sink.
- Event handling:
  - `handle_event(event)` routes `EventMsg` variants: session configuration, agent deltas (message/reasoning), tool calls, approvals, exec command lifecycle, patch apply events, errors, task completion, rate limit updates, and background notifications.
  - Streaming: `handle_streaming_delta`, `handle_stream_finished`, `stream_controller` buffer agent output and convert to `HistoryCell`s; reasoning deltas update status headers and transcript-only cells.
  - Approvals: enqueue `ApprovalRequest`s via bottom pane, manage interrupts through `InterruptManager`.
  - Command execution: tracking `RunningCommand`, building `ExecCell`s, inserting command outputs into history.
  - Rate limits: `RateLimitWarningState` monitors `RateLimitSnapshot` thresholds and queues warning messages.
- User submission:
  - `handle_input_result` processes composer results (user messages, slash commands), performing prompt expansion, image attachments, history insertion, and `CodexOp` submission.
  - Slash commands map to app events (`/status`, `/resume`, `/feedback`, etc.) or model adjustments (model preset, approval preset).
- History/UI updates:
  - `add_to_history` inserts cells into the transcript; `request_redraw` schedules frames when state changes.
  - `session_header` tracks model and displays session metadata in the transcript.
  - Notifications queue when the terminal is unfocused and render on the next draw.
- Ghost commits:
  - `CreateGhostCommitOptions`, `ghost_snapshots` track sandboxed changes, enabling `/undo` and per-turn commit restore.
  - `restore_snapshot` handles rollbacks, updating history cells and bottom pane state.
- Feedback & onboarding:
  - Integrates `codex_feedback` for `/feedback`, stores `feedback` sink to send structured feedback.
  - Handles onboarding banners/welcome messages via `history_cell::new_session_info`.

## Broader Context
- `App` owns a `ChatWidget` and drives its `render`/`handle_event` methods. The widget bridges core events to UI updates and bottom pane interactions, ensuring a cohesive chat loop.

## Technical Debt
- The module is large and multitasks configuration, event handling, UI coordination, and command pipeline. Future refactors could extract dedicated managers (streaming, approvals, slash command execution, ghost commit handling) to reduce complexity.

---
tech_debt:
  severity: high
  highest_priority_items:
    - Split streaming/approval/ghost-commit logic into focused components to make event handling easier to maintain and test.
related_specs:
  - ./chatwidget/agent.rs.spec.md
  - ./chatwidget/interrupts.rs.spec.md
  - ./bottom_pane/mod.rs.spec.md
  - ./exec_cell/mod.rs.spec.md
