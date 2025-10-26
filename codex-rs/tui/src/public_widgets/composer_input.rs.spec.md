## Overview
`ComposerInput` is a thin public wrapper around the internal `ChatComposer`, exposing a widget that captures multi-line input, paste bursts, and submit semantics for reuse by other crates. It surfaces a minimal API for driving key events, rendering, and customizing footer hints.

## Detailed Behavior
- Construction:
  - `new` allocates an internal `ChatComposer` with enhanced key support enabled (Shift+Enter for newlines) and a neutral placeholder (“Compose new task”). It spins up an unbounded channel to receive `AppEvent`s, storing the sender so the composer can trigger events internally.
- Interaction helpers:
  - `input` feeds a `KeyEvent` into the composer and maps the resulting `InputResult` into `ComposerAction::Submitted(String)` when a submission occurs; otherwise returns `None`. Drains any queued app events afterward.
  - `handle_paste` proxies paste handling, returning whether the composer consumed the text and draining events.
  - `flush_paste_burst_if_due` checks whether an in-progress paste burst should be flushed, requesting callers to schedule redraws when it returns true. `recommended_flush_delay` exposes the shared timer interval.
- State accessors:
  - `is_empty`, `clear`, `desired_height`, `cursor_pos`, `is_in_paste_burst` mirror internal composer state.
  - `set_hint_items` and `clear_hint_items` let callers override the footer key hints (each tuple rendered as `<key> <label>`).
- Rendering:
  - `render_ref` delegates to the underlying `ChatComposer`’s `WidgetRef` impl, drawing the input into a provided buffer/area.
- The internal `_tx` keeps the channel sender alive for the lifetime of the widget; `drain_app_events` discards unneeded messages because the public wrapper does not expose them.

## Broader Context
- Designed for external consumers that need Codex’s chat entry experience (cloud tasks, integrations) without instantiating the entire TUI. It packages core behaviors such as paste throttling and keyboard shortcuts into a reusable component.
- Relies on many internal modules (`app_event`, `ChatComposer`), so semver stability should be maintained through this wrapper rather than direct access to internal structures.

## Technical Debt
- The wrapper ignores dispatched `AppEvent`s; if external callers need event visibility (e.g., validation prompts), the API would need to expose a subscription mechanism.
- Channel lifetimes are tied to the widget; integrating with an async runtime or caller-supplied sender could offer more flexibility.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Provide a way to observe composer-generated `AppEvent`s for integrations that care about analytics or telemetry.
related_specs:
  - ../mod.spec.md
  - ../bottom_pane/chat_composer.rs.spec.md
