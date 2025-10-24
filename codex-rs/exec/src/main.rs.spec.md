## Overview
`exec::main` is the binary entrypoint for `codex-exec`. It merges shared CLI overrides, dispatches the special `codex-linux-sandbox` sub-command when invoked via `arg0`, and otherwise delegates to `run_main` in `exec::lib`.

## Detailed Behavior
- Uses `codex_arg0::arg0_dispatch_or_else` to detect whether the executable was invoked as `codex-linux-sandbox`. When that occurs, `codex_arg0` hands back the path to the sandbox shim, and the closure executes the normal CLI flow; the sandbox-specific logic instead lives in the `linux-sandbox` crate (see its spec).
- Parses CLI arguments with `TopCli`, which flattens:
  - `CliConfigOverrides` (shared overrides from `codex_common`); these are spliced into the inner `Cli`’s raw override list so downstream code sees a single source of overrides.
  - The standard `Cli` options defined in `cli.rs`.
- Calls `run_main(inner, codex_linux_sandbox_exe).await`, returning any error to `arg0_dispatch_or_else`, which prints diagnostics and sets exit codes as needed.

## Broader Context
- This module is intentionally thin so that `run_main` remains async-friendly and testable in isolation. The sandbox executable shares the same binary for packaging convenience; its behavior is documented under `linux-sandbox`.
- `CliConfigOverrides` ensures consistent override semantics across CLI tools (e.g., `codex-cli`, `codex-exec`), so changes to override handling should stay aligned with those specs.

## Technical Debt
- None noted; the module’s job is strictly orchestration.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./lib.rs.spec.md
  - ./cli.rs.spec.md
  - ../linux-sandbox/src/lib.rs.spec.md
