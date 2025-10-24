## Overview
`core::tasks::review` runs specialized review turns where the model critiques changes instead of continuing the main conversation. It relies on the shared `run_task` loop but sets `TaskKind::Review` and ensures review mode exits cleanly on abort.

## Detailed Behavior
- `ReviewTask` implements `SessionTask` with `kind()` returning `TaskKind::Review`.
- `run` delegates to `run_task`, which recognizes review mode via the `TurnContext` and isolates history inside the review thread (see `codex.rs`).
- `abort` overrides the default behavior to call `exit_review_mode`, ensuring the session emits the appropriate `ExitedReviewMode` event and clears review-specific state if the task is canceled before completion.

## Broader Context
- Review tasks are triggered by `Op::Review`. They allow users to engage in a temporary branch without affecting the primary conversation history until the review concludes.
- Exiting review mode requires coordination with UI layers; this module’s `abort` hook ensures the user receives a clear signal even when the review is interrupted.
- Context can't yet be determined for multi-review support; currently only one review task may run at a time due to the single-task runner design.

## Technical Debt
- None observed; logic aligns with the shared review flow in `codex.rs`.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.rs.spec.md
  - ../codex.rs.spec.md
