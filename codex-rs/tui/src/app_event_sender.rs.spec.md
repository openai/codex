## Overview
`codex-tui::app_event_sender` wraps the unbounded channel used to deliver `AppEvent`s. It centralizes logging and error handling for outbound events so widgets and background tasks can emit updates without duplicating boilerplate.

## Detailed Behavior
- `AppEventSender` holds an `UnboundedSender<AppEvent>`.
- `new` constructs the sender wrapper.
- `send(event)`:
  - Logs inbound events to `session_log::log_inbound_app_event` (except `CodexOp`, which is logged at submission time) to support session replay tooling.
  - Attempts to send the event on the channel; logs an error via `tracing::error!` if the receiver has been dropped.

## Broader Context
- Widgets (chat composer, bottom pane), file search, and other components clone `AppEventSender` to communicate with `App::handle_event`.
- Session logging relies on this module to capture UI-driven events consistently.
- Context can't yet be determined for backpressure control; current design assumes unbounded channels due to low volume of UI events.

## Technical Debt
- None noted; the wrapper is intentionally small.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./app_event.rs.spec.md
  - ./app.rs.spec.md
