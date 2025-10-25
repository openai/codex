## Overview
`lib.rs` is the runtime hub for the `codex cloud` subcommand. It normalizes backend configuration, initializes auth, provides the `run_exec_command` fast path, and launches the interactive TUI with background tasks that manage task lists, diffs, and apply/preflight operations.

## Detailed Behavior
- Module wiring: exposes `Cli`, re-exports public helpers (`env_detect`, `scrollable_diff`, `util`), and keeps rendering/state modules private.
- `ApplyJob` and `BackendContext` keep task apply state (`task_id`, diff override) and cache the `CloudBackend` plus normalized base URL.
- `init_backend`:
  - Selects between a mock client (`CODEX_CLOUD_TASKS_MODE=mock`) and the HTTP client.
  - Normalizes the base URL, sets the user-agent suffix, and loads credentials with `codex_login::AuthManager`.
  - Configures bearer token and optional `ChatGPT-Account-Id`, logging to `error.log` via `util::append_error_log`.
  - Exits with user-friendly messages when authentication is missing.
- `run_exec_command` implements the `codex cloud exec` CLI: resolves environment id (`resolve_environment_id`), reads prompt from arg/stdin (`resolve_query_input`), posts a task via `CloudBackend::create_task`, and prints a browser-friendly URL (`util::task_url`).
- `resolve_environment_id` fetches environment lists via `env_detect::list_environments`, tolerates label-based lookups, and handles ambiguous or missing selections with descriptive errors.
- `resolve_query_input` gracefully reads prompts from arguments or stdin (with hints when running interactively).
- `spawn_preflight` / `spawn_apply` launch asynchronous tasks that call `CloudBackend::apply_task_preflight` or `apply_task`, update `App` state flags, and emit corresponding `AppEvent`s onto the UI channel. They guard against concurrent preflight/apply requests and schedule spinner refreshes.
- `run_main` is the TUI entrypoint:
  - Parses CLI overrides (currently unused), installs a minimal tracing subscriber, and initializes the backend.
  - Sets up crossterm terminal state (alt screen, raw mode, bracketed paste, keyboard enhancement flags) before constructing a `ratatui` terminal.
  - Initializes `app::App`, logs environment hints, and enqueues initial refresh requests.
  - Spawns background tasks to load tasks, diff details, environment lists, new task submissions, preflight/apply requests, and to poll for spinner updates.
  - Drives an event loop combining crossterm input, ratatui rendering, and responses from the background channel (using `tokio::select!` in the portions beyond this excerpt).

## Broader Context
- Depends on `codex_cloud_tasks_client` abstractions, deferring to that crate for HTTP transport and serialization. Shares auth logic with `codex_core` and `codex_login`.
- Interacts tightly with `app.rs` (state machine), `ui.rs` (rendering), and `env_detect.rs` (environment discovery), keeping the entrypoint focused on orchestration and IO.

## Technical Debt
- None; future enhancements (background enrichment caches, additional modals) are noted in `app.rs` comments but do not affect this module.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./app.rs.spec.md
  - ./cli.rs.spec.md
  - ./env_detect.rs.spec.md
  - ./util.rs.spec.md
