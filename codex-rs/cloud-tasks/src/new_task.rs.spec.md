## Overview
`new_task.rs` encapsulates the state for the “New Task” page in the TUI. It wraps a `codex_tui::ComposerInput` so users can compose prompts, choose environments, and adjust best-of counts without leaving the main interface.

## Detailed Behavior
- `NewTaskPage` stores:
  - `composer` (multi-line text input with shortcuts).
  - `submitting` flag used to disable UI while a submission is in-flight.
  - `env_id` (optional environment identifier).
  - `best_of_n` indicating the selected attempt count.
- `NewTaskPage::new` initializes the composer with hint items (`⏎` send, `Shift+⏎` newline, `Ctrl+O` environment picker, `Ctrl+N` attempts selector, `Ctrl+C` quit).
- `Default` delegates to `new(None, 1)` for convenience.

## Broader Context
- Instances live inside `app::App::new_task` and are rendered by `ui::draw_new_task_page`. Background events update `submitting`, `env_id`, and `best_of_n` based on user actions.

## Technical Debt
- Comments note room for additional helpers as feature work evolves (e.g., validation or state transitions).

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./app.rs.spec.md
  - ./ui.rs.spec.md
