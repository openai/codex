## Overview
`linux_sandbox::linux_run_main` parses sandbox CLI arguments and launches a target command under Landlock filesystem restrictions and seccomp-based network controls. It is the Linux-only counterpart invoked when the shared binary is executed as `codex-linux-sandbox`.

## Detailed Behavior
- `LandlockCommand` (clap `Parser`):
  - `--sandbox-policy-cwd`: root used to resolve relative writable paths.
  - `--sandbox-policy`: serialized `SandboxPolicy` (same type used in Codex core).
  - `command`: trailing var-arg capturing the program and arguments to execute.
- `run_main`:
  - Parses the arguments, applies sandbox restrictions via `landlock::apply_sandbox_policy_to_current_thread`.
  - Validates that a command was provided; otherwise panics.
  - Converts the command vector into `CString`s and calls `libc::execvp`, replacing the current process image.
  - If `execvp` fails, retrieves `errno` and panics with a descriptive message.

## Broader Context
- `apply_sandbox_policy_to_current_thread` (in `landlock.rs`) applies seccomp/Landlock rules based on the supplied `SandboxPolicy`. Tighter sandbox policies originate from `codex-exec` CLI options or Codex configuration.
- Because `execvp` never returns on success, error handling is limited to panics. The shared binary architecture keeps distribution simple while still allowing direct sandbox invocation when necessary.

## Technical Debt
- None significant; error handling is intentional (panic on failure) since the binary is meant to be invoked by trusted tooling.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./landlock.rs.spec.md
  - ../exec/src/lib.rs.spec.md
