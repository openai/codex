## Overview
`app.rs` models the Codex Cloud TUI state machine and defines the internal event protocol exchanged between the UI loop and background workers. It keeps track of task lists, environment filters, apply/preflight dialogs, diff overlays, and “new task” composition state.

## Detailed Behavior
- Environment metadata:
  - `EnvironmentRow` represents rows in the environment selector modal (id, label, pinned flag, repo hints).
  - `EnvModalState` stores modal query/selection.
  - `BestOfModalState` tracks the best-of-N selector.
- Apply workflows:
  - `ApplyResultLevel` classifies preflight/apply outcomes (success/partial/error).
  - `ApplyModalState` records modal state when showing apply results (task id, title, results, skipped/conflict paths, optional diff override).
- `App` aggregates all UI state: task list, selection index, status string, diff overlay, spinners, modals, environment cache, new task page (`new_task::NewTaskPage`), best-of configuration, concurrency flags (`apply_inflight`, `apply_preflight_inflight`), background enrichment bookkeeping (`list_generation`, `in_flight`).
  - Provides navigation helpers (`next`, `prev`), environment-refresh flags, and initialization defaults.
- `load_tasks` performs a timed (`tokio::time::timeout`) fetch via `CloudBackend::list_tasks`, filtering out review-only tasks.
- `DiffOverlay` manages the detail drawer shown when inspecting a task: it owns a `ScrollableDiff`, diff/text/prompt lines, attempt summaries (`AttemptView`), sibling turn ids, and view selection (`DetailView::Diff/Prompt`). Helper methods step through attempts, clamp view flags, and refresh the `ScrollableDiff` content.
- `AttemptView` captures per-attempt metadata (turn id, status, diff/text content, prompt).
- `AppEvent` enumerates all messages the background tasks can send: task list refresh, environment autodetect & list loads, diff/messages results, errors, attempt loading, new task submission responses, and apply/preflight completions.
- Unit tests include a `FakeBackend` to verify environment-aware task filtering.

## Broader Context
- The event enum is consumed by `lib.rs`’s runtime, which spawns background tasks that emit these events. Rendering functions in `ui.rs` read `App` state to draw the TUI.
- Works with `scrollable_diff.rs` to present diff content and `new_task.rs` (composer input) when launching the new-task modal.

## Technical Debt
- Comments note future work: background enrichment caches, diff summary streaming, and additional modal refinements.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Finish the planned background enrichment cache to avoid redundant fetches when scrolling.
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./ui.rs.spec.md
  - ./new_task.rs.spec.md
  - ./scrollable_diff.rs.spec.md
