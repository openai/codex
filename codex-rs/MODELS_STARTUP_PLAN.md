# Models Startup + TUI Gating Plan

Goal: let the TUI render immediately and accept input, while ensuring we do not
lie about the model actually used. Model selection (`/model`) and turn submission
are gated until the session is configured. If `/models` refresh fails, core
keeps using the fallback models seeded at startup (from `models.json`) via the
existing `RwLock`.

This plan is intended to be executed top-to-bottom. Each step is checked off
when completed.

---

## Step 1 - Core: Bound `/models` refresh + prevent duplicate refreshes

- [x] Add a 5s timeout to the remote `/models` request in
      `core/src/models_manager/manager.rs::refresh_available_models`.
  - On timeout/error, do not overwrite `remote_models` (fallback seeded at
    startup remains available automatically).
  - Continue to log failures for visibility.
- [x] Add a `refresh_lock: tokio::sync::Mutex<()>` field to `ModelsManager`.
  - Initialize it in `ModelsManager::new` and `ModelsManager::with_provider`.
  - Acquire it around the refresh logic so concurrent calls do not issue
    duplicate `/models` requests.

Acceptance:
- `refresh_available_models` returns promptly (<= ~5s) even when `/models` hangs.
- Concurrent `list_models` / `get_model` callers do not duplicate the refresh.

---

## Step 2 - Core: Provide a placeholder `ModelFamily` for UI startup

- [x] Add a public placeholder constructor in `core/src/models_manager/model_family.rs`
      (e.g. `ModelFamily::placeholder(&Config) -> ModelFamily`).
  - Must not claim a real model slug; it is only used for pre-session rendering.
  - Uses safe defaults (no reasoning summaries, no parallel tool calls, etc),
    then applies config overrides.

Acceptance:
- TUI can construct a `ModelFamily` without resolving a real model slug.

---

## Step 3 - TUI: Boot without blocking on models

Files:
- `tui/src/app.rs`
- `tui2/src/app.rs`

- [x] Remove startup awaits that currently block the first render:
  - Do not call `ModelsManager::get_model(...).await` during startup.
  - Do not call `ModelsManager::list_models(...).await` during startup.
  - Do not run the existing model migration prompt at startup.
- [x] Construct `ChatWidget` immediately using the placeholder `ModelFamily`.

Acceptance:
- The TUI event loop begins immediately (frame scheduled before any `/models` IO).

---

## Step 4 - TUI: Truthful readiness gating (Loading/Ready only)

Design:
- Ready is defined as "we have received `EventMsg::SessionConfigured`", which
  includes the model actually used.
- While Loading, allow typing, but queue submissions and disable `/model`.

Files:
- `tui/src/chatwidget.rs`
- `tui2/src/chatwidget.rs`

- [x] Stop mutating `config.model` in `ChatWidget::new` based on the
      (placeholder) `ModelFamily`.
  - Start session header with a non-model value like `"Starting..."`.
  - Once `SessionConfigured` arrives, update header from `event.model` (already
    happens in `on_session_configured`).
- [x] Gate turn submission:
  - While not configured, pressing Enter enqueues into the existing
    `queued_user_messages` queue and updates the queued display.
  - Prevent `maybe_send_next_queued_input` from sending anything until the
    session is configured.
  - After session configured:
    - If there is no CLI-provided `initial_user_message`, start draining the
      queued inputs by submitting exactly one (the existing turn-completion loop
      will send the rest sequentially).
    - If there *is* an `initial_user_message`, do not start draining immediately
      (avoid sending queued inputs before the initial message begins a turn).
- [x] Gate `/model`:
  - While not configured, show an info message explaining `/model` is disabled
    until startup completes.

Acceptance:
- Users can type immediately.
- Users can press Enter multiple times during startup; submits are queued and
  later executed in order.
- `/model` is unavailable until the actual model is known (SessionConfigured).

---

## Step 5 - Migration: Schedule for next run (like update)

Goal: never interrupt the user during startup. Migration UX becomes a "pending
notice" rendered on the next run.

- [x] After `SessionConfigured`, compute whether a migration notice should be
      scheduled using the current `ModelsManager` model list (from the `RwLock`).
- [x] Persist a "pending migration notice" to a separate file under `codex_home`
      (similar to `version.json`) so we don’t overcrowd `config.toml`.
- [x] On next run, display the notice as a history cell (non-modal), then clear
      the pending notice (and record it as seen so it won't reappear).

Acceptance:
- No migration prompt blocks startup.
- If migration is relevant, the user sees it next run similarly to update
  notices.

---

## Step 6 - Formatting, lint, and tests

- [x] Run `just fmt` (required after Rust changes).
- [x] Run `just fix -p codex-core` (core changes).
  - Note: in this environment, `just fix` needs to run outside the sandbox
    (`clippy --fix` uses a TCP listener to manage locking).
- [x] Run `just fix -p codex-tui` / `just fix -p codex-tui2` (if those crates changed).
- [x] Run targeted tests:
  - `cargo test -p codex-core`
  - `cargo test -p codex-tui`
  - `cargo test -p codex-tui2` (if changed)
- [ ] Ask before running `cargo test --all-features` (since core changed).

Notes:
- `cargo test -p codex-core` must be run outside the sandbox in this environment
  (wiremock binds an OS port; some seatbelt tests invoke sandbox-exec).
