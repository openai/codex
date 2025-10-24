## Overview
`core::spawn` is a thin wrapper around `tokio::process::Command`. It standardizes how Codex launches child processes for tool executions, ensuring sandbox-related environment variables are set and I/O is configured appropriately.

## Detailed Behavior
- Defines `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` and `CODEX_SANDBOX_ENV_VAR`, which are injected into child environments when network access is disabled or a sandbox is active.
- `StdioPolicy` controls whether the child inherits stdio (`Inherit`) or has stdout/stderr piped with stdin set to null (`RedirectForShellTool`) to avoid blocking on unexpected stdin reads.
- `spawn_child_async`:
  - Logs tracing information about the command, arguments, sandbox policy, and environment.
  - Clears the environment, sets provided variables, and injects `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` when the sandbox policy disallows network access.
  - On Unix, optionally overrides `argv[0]` using `Command::arg0`.
  - Configures stdio according to `StdioPolicy`, setting `kill_on_drop(true)` so orphaned children are terminated.
  - On Linux, installs a `prctl(PR_SET_PDEATHSIG, SIGTERM)` hook to terminate children if Codex exits, and proactively kills the child if the parent already died.
  - Returns the spawned `Child`, which the exec module consumes to stream output.

## Broader Context
- `exec.rs` relies on this function to start shell commands and apply_patch helpers. Centralizing the spawn logic keeps sandbox signalling and stdio defaults consistent across tools.
- Environment variables injected here are used by sandbox wrappers and downstream telemetry to detect sandboxed environments.
- Context can't yet be determined for Windows-specific parent-death signalling; the current implementation relies on Linux-specific `prctl`.

## Technical Debt
- None noted; platform-specific enhancements (e.g., Windows job objects) could be added in the future.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./exec.rs.spec.md
  - ./sandboxing/mod.rs.spec.md
