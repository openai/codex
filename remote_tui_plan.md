# Remote TUI Plan

## Summary
Add remote app-server support to `tui_app_server` so `codex --remote <addr>` launches the app-server-backed TUI against a remote websocket app server instead of an embedded in-process one.

Chosen defaults:
- Keep `--remote <addr>` as a flag, not a bare `remote` token.
- Support remote mode only on `codex` interactive entrypoints for this pass.
- Accept `host:port` and normalize it to `ws://host:port`; also accept explicit `ws://host:port`.
- Require `features.tui_app_server` to be enabled when `--remote` is used; otherwise fail fast.
- In remote mode, skip local cwd mismatch prompting during resume/fork and leave the local TUI cwd unchanged unless `--cd` was explicitly passed.

## Key Changes
### CLI and dispatch
- Add `--remote <addr>` to the top-level interactive CLI surface and thread it through `run_interactive_tui`, `resume`, and `fork`.
- Reject `--remote` on non-interactive subcommands.
- Normalize and validate the address before TUI startup:
  - `host:port` -> `ws://host:port`
  - `ws://host:port` -> unchanged
  - anything else -> fatal error with usage guidance
- If `--remote` is present and `Feature::TuiAppServer` is disabled, return a fatal error rather than falling back to the legacy TUI.

### App-server client transport
- Extend `codex-app-server-client` with a websocket-backed client that performs:
  - `initialize`
  - `initialized`
  - typed request/response handling
  - notification streaming
  - server-request resolution/rejection
  - bounded shutdown/disconnect handling
- Refactor the TUI-facing client boundary to be transport-neutral so `AppServerSession` can wrap either:
  - embedded in-process transport
  - remote websocket transport
- Preserve the current event model expected by `tui_app_server`, including the legacy-notification bridge used by its adapter layer.

### `tui_app_server` remote mode
- Add a startup transport enum for `tui_app_server`:
  - embedded local app server
  - remote websocket app server
- In remote mode:
  - do not start any embedded app-server instances
  - do not use embedded picker/bootstrap helpers
  - connect once to the remote app server and reuse that connection for bootstrap, onboarding, session lookup, and the main runtime
- Reuse the existing app-server-backed flows unchanged over the remote transport:
  - `account/*`
  - `model/list`
  - `thread/start|resume|fork|list|read`
  - `turn/start|steer|interrupt`
  - `review/start`
  - `command/exec`
  - approval / user-input / elicitation server requests
  - config RPCs
- Treat remote mode as server-owned state:
  - onboarding/login state comes from the remote server
  - settings/config views use remote config APIs
  - local trust-screen-only flows are skipped in remote mode

### Resume/fork behavior
- Reuse the existing app-server-backed resume/fork lookup and picker logic, but target the remote app-server session instead of a temporary embedded server.
- In remote mode, do not prompt on local cwd mismatch from remote thread metadata.
- Keep local process cwd unless explicitly overridden with `--cd`.

### Errors and shutdown
- If initial remote connect or initialize fails, show an actionable fatal startup error including the remote address.
- If the remote app server disconnects during runtime, exit the TUI cleanly with a fatal remote-connection error.
- Keep the existing clean quit behavior for `/quit` and `Ctrl+C` in remote mode.

## Public Interfaces
- New top-level CLI flag: `--remote <addr>`
- Accepted forms:
  - `host:port`
  - `ws://host:port`
- New transport-neutral app-server client surface for TUI use
- No app-server protocol changes

## Test Plan
- CLI tests:
  - `codex --remote 127.0.0.1:PORT` selects `tui_app_server` when the feature is enabled
  - `codex resume --remote ... --last` and `codex fork --remote ... --last` flow through remote mode
  - invalid remote addresses fail with the expected message
  - `--remote` with `tui_app_server` disabled fails fast
  - `--remote` on non-interactive subcommands is rejected
- Client transport tests:
  - websocket initialize handshake succeeds
  - typed requests work over websocket
  - notifications and server requests arrive over websocket
  - resolve/reject server request round-trips work
  - disconnects surface as transport errors
- `tui_app_server` tests:
  - remote bootstrap uses remote `account/read`, `model/list`, and `thread/start`
  - remote resume/fork/session picker use remote `thread/list` / `thread/read`
  - remote mode does not start embedded app-server instances
  - remote resume/fork bypasses local cwd prompt
  - clean quit works in remote mode
- End-to-end:
  - connect to websocket app server
  - start/resume/fork a thread
  - submit a turn and stream notifications
  - handle approvals or user-input requests
  - quit cleanly

## Assumptions
- First pass targets the existing websocket transport only.
- Standalone `codex-tui` remains local-only for now.
- Remote mode is fully app-server-owned: auth, config RPCs, thread metadata, approvals, and history all come from the remote server.
