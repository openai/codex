## Overview
`chatwidget::agent` spawns async tasks that bootstrap a Codex conversation and forward operations/events between the UI and `codex-core`.

## Detailed Behavior
- `spawn_agent(config, app_event_tx, server)`:
  - Creates an `UnboundedSender<Op>`/receiver pair.
  - Starts a task that calls `ConversationManager::new_conversation`, forwards the initial `SessionConfigured` event to the UI, and then:
    - Spawns a loop to submit incoming ops to the conversation.
    - Streams events from `conversation.next_event()` back to the UI via `AppEvent::CodexEvent`.
- `spawn_agent_from_existing(conversation, session_configured, app_event_tx)` does the same for an already-established conversation, sending the provided `SessionConfiguredEvent` immediately before looping.
- Errors during bootstrap or op submission are logged with `tracing::error`.

## Broader Context
- `ChatWidget` invokes these helpers to start new conversations or resume existing ones (e.g., backtrack forks).

## Technical Debt
- None; module intentionally keeps the glue thin and delegates error messaging to the main UI.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../chatwidget.rs.spec.md
