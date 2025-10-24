## Overview
`core::sandboxing` converts portable command descriptions into sandbox-ready execution environments. It decides which sandbox to use, augments commands with sandbox wrappers, and exposes utilities for detecting sandbox denials.

## Detailed Behavior
- `CommandSpec` captures the portable command representation (program, args, cwd, environment overrides, timeout, escalation metadata).
- `ExecEnv` is the transformed form consumed by `exec`: it includes the final command vector (with sandbox wrapper when applicable), merged environment, timeout, selected `SandboxType`, escalation metadata, and optional `arg0`.
- `SandboxPreference` (legacy) and `SandboxablePreference` (from tool runtimes) influence sandbox selection. `SandboxManager::select_initial` chooses the sandbox based on preference and `SandboxPolicy`, favouring seatbelt on macOS and seccomp on Linux unless policies demand otherwise.
- `SandboxManager::transform`:
  - Clones command arguments and extends them with sandbox wrappers when needed:
    - macOS seatbelt: prefixes with the seatbelt executable and adds `CODEX_SANDBOX_ENV_VAR=seatbelt`.
    - Linux seccomp: requires `codex-linux-sandbox` path (returning `MissingLinuxSandboxExecutable` if absent) and sets `arg0` override for process titles.
  - Inserts `CODEX_SANDBOX_NETWORK_DISABLED` when the policy disallows network access.
  - Returns an `ExecEnv` ready for execution.
- `SandboxManager::denied` reuses `exec::is_likely_sandbox_denied` to interpret command failures.
- `execute_env` simply clones and forwards the `ExecEnv` to `execute_exec_env`, keeping the API symmetrical.

## Broader Context
- Tool runtimes call into `SandboxManager` via the orchestrator to prepare commands before execution. This module ensures sandbox policies apply uniformly regardless of the originating tool.
- Seatbelt and landlock helpers (`create_seatbelt_command_args`, `create_linux_sandbox_command_args`) live in their respective platform modules, so updates to sandbox policy formats must maintain compatibility here.
- Context can't yet be determined for Windows sandboxing; current behaviour fakes `SandboxType::None` on non-supported platforms.

## Technical Debt
- None recorded; future sandbox integrations (e.g., Windows) would extend `SandboxType` and the selection logic.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../exec.rs.spec.md
  - ../tools/orchestrator.rs.spec.md
  - ../tools/sandboxing.rs.spec.md
