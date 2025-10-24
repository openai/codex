## Overview
`codex-cli::exit_status` normalizes child-process exit handling for sandbox commands. It propagates the child’s exit code (or signal) back to the parent process so shell invocations receive accurate statuses.

## Detailed Behavior
- Unix implementation:
  - Uses `ExitStatusExt` to inspect signals.
  - If the child exited normally, calls `std::process::exit(code)`.
  - If terminated by a signal, exits with `128 + signal` (POSIX convention).
  - Fallback exit code `1` if neither applies (rare).
- Windows implementation:
  - Exits with the child’s code when available, otherwise defaults to `1`.
- Both functions are marked `-> !`, ensuring the parent process terminates immediately after handling the status.

## Broader Context
- Shared by sandbox subcommands (`debug_sandbox.rs`), keeping CLI behavior aligned with shell expectations when wrapping commands.
- Logic mirrors patterns used in other Codex binaries to retain consistent exit semantics.

## Technical Debt
- None; implementations are minimal and platform-aware.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./debug_sandbox.rs.spec.md
