# Agent Switching Flow (Implemented)

This document summarises the current “enter delegate session” behaviour that shipped alongside follow-up support on 2025‑10‑20. Users can jump into a finished delegate conversation, interact directly, and return to the primary agent—all without leaving the TUI.

---

## User Experience

1. **Launch picker** – `/agent` (or the corresponding UI shortcut) opens the delegate picker via `ChatWidget::open_delegate_picker` (`codex-rs/tui/src/chatwidget.rs`). The picker lists reusable sessions and detached runs, highlighting the active session if the user is already inside a delegate.
2. **Enter session** – Selecting “Enter session” for a delegate sends `AppEvent::EnterDelegateSession(conversation_id)` (`chatwidget.rs:2207`). `App::activate_delegate_session` (`codex-rs/tui/src/app.rs:1250`) switches the active `SessionHandle` to the chosen conversation, hydrates it from the shadow cache if available, and routes subsequent user input to that sub-agent.
3. **While inside** – The composer banner shows the delegate name; history updates stream directly from that delegate’s `CodexConversation`. Switching again simply selects the new session from the picker.
4. **Return to primary** – Choosing “Return to main agent” issues `AppEvent::ExitDelegateSession` (`chatwidget.rs:2147`). `App::return_to_primary` (`codex-rs/tui/src/app.rs:900`) restores the primary session, logs a summary message, and leaves the delegate conversation available for future follow-ups.

Key UI touches:
- Delegate history remains isolated; no events leak into other sessions.
- Summaries and status indicators update the delegate tree so the user can see which runs generated additional interaction.
- Errors during entry/exit bubble into history cells and log via `tracing::error!`.

---

## Orchestrator Integration

- `AgentOrchestrator::enter_session` (`codex-rs/multi-agent/src/orchestrator.rs:960`) returns `ActiveDelegateSession` containing the session summary, live `CodexConversation`, a session-configured snapshot, and the per-session event receiver.
- `AgentOrchestrator::dismiss_session` removes a reusable session when the user chooses “Dismiss”; it refuses if a run is active to avoid mid-stream exits.
- Shadow snapshots hydrate the conversation instantly. Missing snapshots fall back to rollout replay with an informative banner.
- Parent/child relationships remain intact—switching doesn’t alter delegate lineage or follow-up behaviour.

---

## Known Limitations / Future Work

- No dedicated breadcrumbs in the main transcript yet; summaries appear as text cells. UX improvements (timers, richer chips) are tracked in `ai-temp/ui-ux-delegation.md`.
- We currently keep the delegate conversation alive indefinitely until dismissed; eviction policy may be revisited alongside shadow retention rules.

Despite these gaps, switching is fully functional and covered by the orchestrator/TUI tests. Consult `codex-rs/tui/src/app.rs`, `chatwidget.rs`, and `ai-temp/ui-ux-delegation.md` for deeper details.
