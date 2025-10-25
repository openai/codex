## Overview
`ui.rs` renders the Codex Cloud TUI using `ratatui`. It draws the task list, footers, modals, overlays, and the “New Task” page, reacting to `app::App` state and keeping styling consistent with the wider Codex terminal UI.

## Detailed Behavior
- `draw` orchestrates the frame: splits the screen into list/content plus footer, draws either the task list or new-task page, then overlays diff/environment/best-of/apply modals when present.
- Helper geometry/styling:
  - `rounded_enabled`, `overlay_outer`, `overlay_block`, and `overlay_content` compute modal rectangles and apply optional rounded borders (`CODEX_TUI_ROUNDED` flag).
- `draw_new_task_page` renders the composer UI with hints, environment label, and attempts summary; manages cursor placement based on `codex_tui::ComposerInput`.
- `draw_list` builds the task list `List` widget, dims background when modals are active, displays selection titles and percent scrolled, and injects status icons based on task state.
- `render_task_item` (later in file) formats list rows with timestamps, status badges, environment labels, and partial diff preview metrics.
- Other renderers include:
  - `draw_footer` (toolbar/status), `render_help_spans`, and spinner helpers.
  - `draw_diff_overlay` for the detailed diff/prompt view, leveraging `ScrollableDiff`.
  - `draw_env_modal`, `draw_best_of_modal`, and `draw_apply_modal` for interactive dialogs with headings, list selections, and results (skipped/conflict path lists).
  - `draw_attempt_tabs` for switching between sibling turn attempts, coloring statuses using `AttemptStatus`.
- Uses `render_markdown_text` from `codex_tui` to display assistant output with formatting.
- Relying on `OnceLock` caches, the module avoids recomputing static border configuration or timestamp formatters.

## Broader Context
- Consumes `app::App` state mutated by `lib.rs` event loop and background tasks, and updates `app` (e.g., selection changes) inline during rendering when required.
- Shares design patterns with the main Codex TUI (stylize helpers, footers, percent scrolled indicators) to provide a familiar UX.

## Technical Debt
- Comments indicate future work for richer diff summarization and background enrichment; current rendering already accommodates placeholders for these features.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./app.rs.spec.md
  - ./scrollable_diff.rs.spec.md
  - ./new_task.rs.spec.md
