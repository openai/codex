## Overview
`linux-sandbox::lib` exposes the `run_main` entrypoint for the `codex-linux-sandbox` binary. It dispatches to the Linux-specific implementation (`linux_run_main`) when running on Linux, and panics on unsupported platforms to signal misconfiguration.

## Detailed Behavior
- `run_main` (Linux): simply delegates to `linux_run_main::run_main`, which never returns (`-> !`).
- `run_main` (non-Linux): panics immediately, because the sandbox executable is only meaningful on Linux hosts.

## Broader Context
- `codex-exec` shares a binary with the sandbox; `exec::main` detects the arg0 `codex-linux-sandbox` case and calls this module. Specifications for `linux_run_main` and `landlock` cover the actual sandbox enforcement and command execution.

## Technical Debt
- None; the module is purely a platform dispatch shim.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./linux_run_main.rs.spec.md
  - ./landlock.rs.spec.md
  - ../exec/src/main.rs.spec.md
