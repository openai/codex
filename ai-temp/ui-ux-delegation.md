# Delegation UI & UX Notes

## Current Flow
- Primary agent streaming uses `StreamController` to animate delta lines (`codex-rs/tui/src/chatwidget.rs:698`, `streaming/controller.rs:9`). The controller emits `AgentMessageCell` entries and drives the commit animation via `AppEvent::StartCommitAnimation`.
- Exec and MCP tool calls rely on dedicated history cells (`ExecCell`, `new_active_mcp_tool_call`) with live updates for begin/end events (`chatwidget.rs:633`, `chatwidget.rs:909`).
- Delegation events from the orchestrator reach `App::handle_delegate_update` (`codex-rs/tui/src/app.rs:446`). `DelegateEvent::Delta` now streams sub-agent output through the same `StreamController` pipeline, while start/completion still use `add_info_message`/`add_delegate_completion` for context. Incoming events carry run depth so the chat history can render indented entries for nested delegates.
- `DelegateEvent::Started` activates the bottom-pane status indicator with a “Delegating to #<agent>` header and hides it once the run finishes (`codex-rs/tui/src/chatwidget.rs:2165-2196`), reducing ambiguity about who is currently working.

## Observed Gaps
- No transcript linking: once the delegate finishes, the TUI shows the final answer but lacks a quick way to drill into the delegate’s own session (the path is only available in logs).
- Duration is implicit: the status header flips back to “Working” when delegation ends, but we still do not surface elapsed time or a final summary chip in the transcript.
- Nested runs only show progress via indented info messages; we may still want richer breadcrumbs or timers in the status widget.

## UX Goals
1. **Live streaming** – continue to reuse `StreamController`, but add safeguards against duplicate completions (covered by the new test) and consider showing a collapsed summary once the stream ends.
2. **Session breadcrumbs** – insert a history cell with the delegate’s session ID and an action (e.g., `/delegate-open <id>`) to reopen or inspect the sub-agent log.
3. **Status context** – enhance the existing “Delegating to #…” banner with elapsed time and/or a persistent history chip that points back to the sub-agent run.

## Implementation Notes
- Delegation already reuses `StreamController` via `DelegateEvent::Delta`; keep the plumbing local to `ChatWidget` so other surfaces can opt-in without pulling UI dependencies into the orchestrator.
- Add new history cell types (e.g., `DelegateStartCell`, `DelegateSummaryCell`) to avoid overloading existing exec/info cells.
- Propagate failure (`DelegateEvent::Failed`) into a red error history cell and optionally a notification (see `Notification::AgentTurnComplete` in `chatwidget.rs:1871` for pattern).
- Update `status/helpers.rs` so `/status` lists active/past delegates with timestamps.

## Outstanding Questions
- Should delegate output merge into the primary transcript, or display in a collapsible block to avoid clutter?
- Do we expose a command to jump into the delegate’s rollout file (`codex_core::ConversationManager::resume_conversation_from_rollout`) from the UI?
- How do we handle nested delegation (delegate triggering its own sub-delegate)? Requires queueing and UI affordances.

These design notes align with the wiring in `codex-rs/tui/src/app.rs`, `codex-rs/tui/src/chatwidget.rs`, and the orchestrator in `codex-rs/multi-agent/src/orchestrator.rs`. Further iterations should focus on breadcrumbs, elapsed-time surfacing, and tighter transcript integration now that delta streaming and status updates are in place.
