## Overview
`lib.rs` adds a cancellation-aware extension trait for futures. It lets callers await a future with a `tokio_util::sync::CancellationToken`, returning a dedicated `CancelErr` when the token fires.

## Detailed Behavior
- `CancelErr::Cancelled` distinguishes cooperative cancellation from other errors.
- `OrCancelExt` defines an async method `or_cancel` that any `Future + Send` can use.
- Implementation uses `tokio::select!` to race the future with `token.cancelled()`, returning `Err(CancelErr::Cancelled)` when the token triggers first.
- Unit tests cover completion-before-cancel, cancellation after a delay, and pre-cancelled tokens.

## Broader Context
- Used across Codex async workflows to support graceful shutdown and request cancellation without wiring manual select loops.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
