## Overview
`codex-cloud-tasks` implements the `codex cloud` subcommand: a TUI for browsing Codex Cloud tasks plus a thin CLI for submitting new jobs. It integrates with the `codex-cloud-tasks-client`, detects environments, renders diffs/prompts, and drives apply/preflight workflows against the backend.

## Detailed Behavior
- `src/lib.rs` owns startup wiring: argument parsing, auth/bootstrap (`init_backend`), CLI execution paths, TUI runtime, and background task orchestration for apply/preflight.
- `src/app.rs` models TUI state (`App`, modals, overlays) and defines the internal event protocol (`AppEvent`) used by background tasks.
- `src/cli.rs` exposes the Clap-based interface (`codex cloud` and `codex cloud exec`).
- `src/env_detect.rs` discovers likely environments using Git remotes and backend projections.
- `src/new_task.rs`, `src/scrollable_diff.rs`, `src/ui.rs`, and `src/util.rs` provide supporting widgets, rendering, and shared helpers.

## Broader Context
- The crate acts as the human-facing client for Codex Cloud tasks, sitting on top of the `codex-cloud-tasks-client` abstraction and reusing Codex’s auth/token plumbing.
- Shares TUI utilities with `codex-tui` (composer input, markdown rendering) to keep UX consistent with the main Codex terminal app.

## Technical Debt
- None tracked within the crate; comments note future feature work (background enrichment caching, additional apply dialogs).

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/app.rs.spec.md
  - ./src/cli.rs.spec.md
  - ./src/env_detect.rs.spec.md
  - ./src/new_task.rs.spec.md
  - ./src/scrollable_diff.rs.spec.md
  - ./src/ui.rs.spec.md
  - ./src/util.rs.spec.md
