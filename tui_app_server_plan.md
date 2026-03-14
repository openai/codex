# TUI App-Server Migration Plan

## Summary
- Create a new sibling crate `codex-rs/tui_app_server` as the experimental app-server-backed TUI, starting from a copy of the current `codex-rs/tui`.
- Keep `codex-rs/tui` intact as the legacy compatibility path.
- Add a new config-level feature flag in `codex-rs/core/src/features.rs`, off by default, to choose between legacy `tui` and experimental `tui_app_server`.
- Route selection at runtime after config load. Do not use a compile-time Cargo feature for backend selection.
- The `tui_app_server` implementation must not depend on or access shared `AuthManager` or `ThreadManager`. All auth/thread/session state must come from app-server APIs and notifications.

## Runtime Selection
- Add `Feature::TuiAppServer` in `codex-rs/core/src/features.rs`.
- Use a config key such as `tui_app_server`, default `false`, stage `Experimental`.
- Load config before interactive startup dispatch in:
  - `codex-rs/cli/src/main.rs`
  - `codex-rs/tui/src/main.rs`
- After config load, branch at runtime:
  - feature off: call legacy `codex_tui::run_main`
  - feature on: call `codex_tui_app_server::run_main`
- Keep non-interactive commands unchanged.
- Keep both crates built all the time; selection is runtime-only.

## Separation Constraints
- `tui_app_server` must not store, receive, or query:
  - `AuthManager`
  - `ThreadManager`
  - direct `codex_protocol::protocol::Op`
  - direct `codex_protocol::protocol::EventMsg`
- `tui_app_server` must not call bootstrap escape hatches like:
  - `app_server.auth_manager()`
  - `app_server.thread_manager()`
- In `tui_app_server`, account/thread/session state must be derived from:
  - `account/read`
  - `account/rateLimits/read`
  - `thread/start`
  - `thread/resume`
  - `thread/fork`
  - `thread/read`
  - `thread/list`
  - app-server notifications
  - app-server server requests
- Shared code between `tui` and `tui_app_server` is allowed only for backend-agnostic rendering/utilities. Do not introduce a shared core-manager abstraction layer.

## App-Server Migration Scope
- Introduce an `AppServerSession` facade in `tui_app_server` over `InProcessAppServerClient`.
- Drive the new TUI entirely through:
  - typed app-server requests
  - app-server notifications
  - app-server server-request resolution/rejection
- Replace direct core-driven flows with app-server equivalents:
  - submit turn: `turn/start`
  - steer: `turn/steer`
  - interrupt: `turn/interrupt`
  - set thread name: `thread/setName`
  - rollback: `thread/rollback`
  - compact: `thread/compact/start`
  - clean background terminals: `thread/backgroundTerminals/clean`
  - review: `review/start`
  - realtime: `thread/realtime/*`
  - auth/login/logout: `account/*`
  - config edits: `config/read`, `config/value/write`, `config/batchWrite`
  - `!cmd`: `command/exec` and related output/write/terminate/resize APIs
- Set `persist_extended_history = true` on `thread/start`, `thread/resume`, and `thread/fork` in `tui_app_server` so transcript reconstruction uses rich `ThreadItem` history.
- Port resume/fork picker to app-server thread APIs instead of rollout/core thread listing.
- Port status/onboarding/account views to app-server account/rate-limit/model/config data instead of `AuthManager`.

## Unsupported Features To Stub
The following TUI features are not yet exposed through app-server APIs and should be stubbed with explicit user-facing errors in `tui_app_server`:
- Custom prompt listing/picker: current `ListCustomPrompts`
- MCP tool inventory view: current `ListMcpTools`
- Composer history persistence/fetch: current `AddToHistory` and `GetHistoryEntryRequest`
- Memory maintenance shortcuts: current `DropMemories` and `UpdateMemories`

Stub behavior requirements:
- Keep UI entrypoints visible only if removing them would create excess churn.
- Use a consistent message such as: `Not available in app-server TUI yet.`
- Log the stub hit so follow-up API work can be prioritized.

## Validation
- Add config/feature tests for `Feature::TuiAppServer`.
- Add runtime dispatch tests proving interactive startup selects the correct crate based on config.
- Add `tui_app_server` tests for:
  - thread/turn/item notification reducers
  - approval and input server-request handling
  - resume/fork transcript reconstruction with `persist_extended_history = true`
  - status/account/onboarding rendering
  - unsupported-feature stub behavior
- Add a regression check that `tui_app_server` does not depend on `AuthManager` or `ThreadManager`.
- Keep legacy `tui` behavior unchanged when the flag is off.

## Progress Notes
- 2026-03-14: `AppServerSession` exists and startup bootstrap now uses `account/read`, `account/rateLimits/read`, `model/list`, and app-server `thread/start` for fresh sessions. Resume/fork, server requests, and notification-driven state are still pending, so the manager-removal checklist items remain intentionally unchecked.
- 2026-03-14: Custom prompt listing is now explicitly stubbed in `tui_app_server`: the crate no longer submits `ListCustomPrompts`, `/prompts:...` now surfaces `Not available in app-server TUI yet.`, and unexpected `ListCustomPromptsResponse` events are ignored with a warning.
- 2026-03-14: `/status` in `tui_app_server` no longer depends on `AuthManager`; startup bootstrap now seeds a plain account display + plan type from `account/read`, and `ChatWidget` threads that app-server-derived state into status/session info. Other account/onboarding flows still use `AuthManager`.
- 2026-03-14: Fresh startup, CLI resume, and CLI fork now all create their primary thread through app-server `thread/start`, `thread/resume`, and `thread/fork` with `persist_extended_history = true`, then attach only an op-forwarder for the legacy widget path. Live thread events are now sourced from app-server `LegacyNotification`s instead of direct `thread.next_event()` listeners for the primary startup path.
- 2026-03-14: Approval, permissions, request-user-input, and MCP elicitation prompts are now bridged through app-server `ServerRequest`s. `tui_app_server` records incoming request ids, correlates MCP elicitation requests with the legacy overlay events, resolves the app-server request when the existing overlay emits the matching legacy `Op`, and rejects unsupported request types like dynamic tool calls with explicit user-facing errors.
- 2026-03-14: In-app `NewSession`, resume-picker resume, and `/fork` now use app-server `thread/start`, `thread/resume`, and `thread/fork` instead of `ThreadManager` lifecycle helpers. The crate still uses `ThreadManager::get_thread` to attach an op forwarder to the started thread, so the manager-removal checklist items remain unchecked.
- 2026-03-14: Non-picker startup session selection now uses app-server `thread/read` and `thread/list` instead of rollout/session-index lookups for `resume --last`, `fork --last`, `resume <id|name>`, and `fork <id|name>`. Startup `resume`/`fork` picker selection now pages through app-server `thread/list`, and the in-app resume picker action now starts a temporary embedded app-server session so it also uses the same app-server picker path.
- 2026-03-14: Primary `tui_app_server` sessions no longer rebuild `ChatWidget` around `spawn_op_forwarder(...)`. `ChatWidget` can now emit `AppEvent::CodexOp` directly, `App` owns primary-thread submission, and fresh/start/resume/fork/session-switch rebuilds now use the app-event-backed constructor. Active-thread dispatch now routes `turn/start`, `skills/list`, `review/start`, `thread/compact/start`, `thread/rollback`, `thread/name/set`, `thread/backgroundTerminals/clean`, and `thread/realtime/*` through `AppServerSession` before falling back to direct core thread submission for the still-unported ops.
- 2026-03-14: Active-thread state now tracks the in-flight turn id inside the per-thread event store, so the primary runtime can route `Op::Interrupt` through app-server `turn/interrupt` instead of blindly falling back to direct core submission. The app-server TUI also now runs `!cmd` via background `command/exec` requests using a cloneable in-process request handle, bridges `command/exec/outputDelta` notifications back into legacy `ExecCommandOutputDelta` events, and synthesizes the matching `TurnStarted`, `ExecCommandBegin`, `ExecCommandEnd`, and `TurnComplete` events for the existing widget path.
- 2026-03-14: `tui_app_server/src` no longer references `AuthManager`, `ThreadManager`, or their bootstrap escape hatches. Account bootstrap, rate-limit updates, and onboarding now derive from app-server account APIs and notifications, with device-code ChatGPT login explicitly stubbed as unavailable in the app-server TUI path. A regression test now scans `tui_app_server/src` to keep manager escape hatches from creeping back in.
- 2026-03-14: Active-thread `Op::UserTurn` now routes through app-server `turn/steer` whenever the thread already has an in-flight turn id, and through `turn/start` otherwise. Review, compact, rollback, thread naming, background-terminal cleanup, realtime conversation APIs, and legacy-TUI fallback verification are all now covered by code paths or test runs in this branch.
- 2026-03-14: Direct legacy `Op` emission has been reduced in leaf UI modules. Status interrupt, request-user-input submission/interrupt, skills refresh, realtime audio frames, thread renaming, compact, review popups, approval decisions, and MCP/app-link elicitation responses now funnel through `AppEventSender` helpers instead of constructing protocol ops inline across those widgets. The remaining direct `Op` / `EventMsg` work is concentrated in the main app/chatwidget bridge and legacy replay adapter files.
- 2026-03-14: The remaining app-side `Op` bridge is now isolated behind `tui_app_server/src/app_command.rs`. Runtime submission, pending interactive replay bookkeeping, and app-server request resolution all translate through `AppCommand` helpers instead of matching on raw protocol ops across the app layer. `cargo test -p codex-tui-app-server` passed after this extraction. Direct `EventMsg` consumption is still the one unchecked bridge item.

## Working Rules
- The implementing agent should keep this file updated as the source of truth.
- The implementing agent should check off items in the master checklist as work completes.
- If the thread compacts, the next agent should resume from this file first and continue updating the checklist in place.

## Assumptions
- Runtime selection should apply to both `codex` interactive startup and standalone `codex-tui`.
- The new flag remains off by default until parity is good enough to broaden usage.
- Missing app-server surfaces should be stubbed in this branch, not added here unless separately requested.

## Master Checklist
- [x] Add `Feature::TuiAppServer` and `tui_app_server` config flag in `codex-rs/core/src/features.rs`
- [x] Add `codex-rs/tui_app_server` as a sibling workspace crate
- [x] Add runtime interactive dispatch in `codex-rs/cli/src/main.rs`
- [x] Add runtime interactive dispatch in `codex-rs/tui/src/main.rs`
- [x] Ensure both TUI crates expose compatible interactive entrypoints
- [x] Remove all `AuthManager` usage from `tui_app_server`
- [x] Remove all `ThreadManager` usage from `tui_app_server`
- [x] Remove direct `Op` submission plumbing from `tui_app_server`
- [ ] Remove direct `EventMsg` consumption plumbing from `tui_app_server`
- [x] Add `AppServerSession` facade in `tui_app_server`
- [x] Port startup/account/rate-limit/model bootstrap to app-server APIs
- [x] Port thread start/resume/fork/read/list flows to app-server APIs
- [x] Enable `persist_extended_history` for app-server-backed thread lifecycle calls
- [x] Port turn start/steer/interrupt flows to app-server APIs
- [x] Port approval, permissions, tool input, and MCP elicitation handling to app-server server requests
- [x] Port `!cmd` handling to `command/exec` APIs
- [x] Port review flow to `review/start`
- [x] Port compact flow to `thread/compact/start`
- [x] Port rollback flow to `thread/rollback`
- [x] Port thread naming to `thread/setName`
- [x] Port background terminal cleanup to `thread/backgroundTerminals/clean`
- [x] Port realtime flow to `thread/realtime/*`
- [x] Port resume/fork picker to app-server thread APIs
- [x] Port status/account/onboarding views to app-server-derived state
- [x] Stub custom prompt listing with user-facing error
- [x] Stub MCP tool inventory with user-facing error
- [x] Stub composer history persistence/fetch with user-facing error
- [x] Stub memory maintenance shortcuts with user-facing error
- [x] Add config/feature tests for `Feature::TuiAppServer`
- [x] Add runtime dispatch tests for legacy vs app-server TUI selection
- [x] Add `tui_app_server` reducer/unit tests
- [x] Add `tui_app_server` UI snapshot coverage where output changes are user-visible
- [x] Add regression checks that `tui_app_server` does not depend on `AuthManager` or `ThreadManager`
- [x] Verify legacy `tui` remains unchanged when the feature flag is off
- [x] Keep this checklist updated in-place while implementing
