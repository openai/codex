## Overview
`linux-sandbox::main` is a thin binary entrypoint that delegates to `codex_linux_sandbox::run_main()`. It exists so the sandbox logic can live in a library crate while providing a runnable binary.

## Detailed Behavior
- Calls `codex_linux_sandbox::run_main()` and never returns (`!`), inheriting cwd/env/argv behavior from the caller as noted in the comment.

## Broader Context
- Used by the Codex execution pipeline to launch commands inside the Linux sandbox; the heavy lifting is documented in the `codex-linux-sandbox` library specs.

## Technical Debt
- None; the entrypoint intentionally defers logic to the library crate.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ../../core/src/unified_exec/session_manager.rs.spec.md
