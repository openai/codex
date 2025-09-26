# ExecFlow Supervisor

ExecFlow is the supervisory layer that now wraps every `exec_command`
invocation. It turns long‑running shell commands into managed sessions with
explicit lifetimes, logging, and control signals. The goal is to guarantee that
agents never leave “runaway” processes behind while still supporting legitimate
long builds or interactive flows.

## Motivation

Previously, commands launched by the agent were effectively fire‑and‑forget:

- a background `npm start` or `sleep` could run forever unless a human
  intervened;
- idle terminals consumed sandbox resources and blocked subsequent commands;
- truncating large stdout streams relied on ad hoc token budgets, making UX
  inconsistent across clients.

ExecFlow consolidates the guardrails for these issues: every session now has an
explicit idle watchdog, a hard deadline, predictable logging, and a bidirectional
control channel. This keeps contributions reviewable and protects shared compute
while preserving the ability to run legitimate long tasks.

## Default Behaviour

- **Idle watchdog.** Each session receives an `idle_timeout_ms` (default 5 minutes).
  If no stdout/stderr chunks *and* no keepalive signals arrive within that
  window, ExecFlow issues `Ctrl-C`, waits for `grace_period_ms` (default 5 s) and
  escalates to `SIGKILL` if the process is still running.
- **Hard deadline.** A second guardrail, `hard_timeout_ms` (default 2 hours),
  terminates the process even if it is still producing output. The same
  Ctrl-C/kill ladder is used.
- **Log redirection.** Once combined stdout/stderr exceeds `log_threshold_bytes`
  (default 4 KiB), the remainder is streamed to a dedicated log file
  `~/.cache/codex/exec_logs/session-<id>-<timestamp>.ansi`. The initial chunk is
  still returned inline so the model can show context without exhausting tokens.
- **Session registry.** ExecFlow keeps an in-memory registry of sessions
  (`Running`, `Grace`, `Terminated`). The new `list_exec_sessions` tool provides a
  compact textual snapshot suitable for UI surfaces and agents.

## Tools Exposed to the Model

| Tool name              | Purpose                                                       |
|------------------------|---------------------------------------------------------------|
| `exec_command`         | start a PTY-backed command (unchanged API)                    |
| `write_stdin`          | send stdin to an existing session                             |
| `exec_control`         | manage a session: keepalive, interrupt, terminate, force kill |
| `list_exec_sessions`   | dump a concise overview of active/recent sessions             |

### `exec_control`

```json
{
  "session_id": 3,
  "action": { "type": "keepalive", "extend_timeout_ms": 120000 }
}
```

Allowed `action.type` values:

| Type             | Effect                                                                 |
|-----------------|-------------------------------------------------------------------------|
| `keepalive`      | Record activity; optional `extend_timeout_ms` overwrites idle timeout.  |
| `send_ctrl_c`    | Inject ASCII `0x03` through the PTY.                                    |
| `terminate`      | Enter grace mode and send `Ctrl-C`.                                     |
| `force_kill`     | Immediately kill the process (via `ChildKiller`).                      |
| `set_idle_timeout` | Update idle timeout for future watchdog checks without keepalive.    |

Responses contain `status` (`ack`, `no_such_session`, `already_terminated`,
`reject(...)`) and an optional explanatory `note`.

### `list_exec_sessions`

Returns human-readable lines of the form:

```
#03 running uptime=47.2s idle_left=212.8s bytes=8192 log=/…/session-00000003-….ansi cmd=npm run build
```

This allows the UI or the model to detect hanging commands without fetching
large logs.

## SessionManager API Additions

`SessionManager` now exposes two new entry points:

- `handle_exec_control_request(ExecControlParams)` – executes the `exec_control` actions.
- `list_sessions() -> Vec<ExecSessionSummary>` – returns snapshots for
  `list_exec_sessions`.

`ExecSessionSummary` captures the session id, preview of the original command,
`SessionLifecycle` (`Running`/`Grace`/`Terminated`), uptime, time left before the
idle watchdog fires, total bytes emitted, and the optional log path.

## Log Storage

- Location: `~/.cache/codex/exec_logs/` (Linux/macOS) or `%TEMP%\codex-exec-logs`
  (Windows). The directory is created on-demand.
- Format: raw ANSI stream with the file name `session-<id>-<timestamp>.ansi`.
- Integrity: SHA-256 is reported with the inline response so downstream tooling
  can verify the log file if needed.

## Configuration Surface

`ExecCommandParams` understands four new knobs:

| Field                 | Default   | Notes                                                 |
|-----------------------|-----------|-------------------------------------------------------|
| `idle_timeout_ms`     | 300_000   | Minimum 1 000 ms, maximum 24 h                        |
| `hard_timeout_ms`     | 7_200_000 | Optional; disables hard deadline if set to `None`     |
| `grace_period_ms`     | 5_000     | Pause between `Ctrl-C` and escalation                 |
| `log_threshold_bytes` | 4 * 1024  | Switch-over point from inline output to log file      |

All fields are optional in JSON inputs; safe defaults are applied automatically.

## Testing

Two targeted async tests cover the critical flows:

- `idle_timeout_terminates_session` – ensures a silent `sleep` is cleaned up.
- `keepalive_extends_session` – validates that keepalive signals keep a long job
  alive and reachable.

Full regression coverage is reachable via:

```bash
cargo test -p codex-core session_manager
cargo test -p codex-core --all-features
```

## Operational Notes

- The watcher loops run inside the Tokio runtime and prune finished sessions
  automatically; logs older than 10 minutes after termination are dropped.
- `force_kill` uses `portable_pty::ChildKiller` directly, so behaviour is
  uniform across Unix platforms and Windows.
- The registry is in-memory today; if CLI restarts are a requirement, future
  work can persist the session metadata and reattach after launch.
