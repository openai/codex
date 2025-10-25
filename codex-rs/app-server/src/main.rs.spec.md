## Overview
`codex-app-server`’s binary entrypoint wires CLI execution through the shared `arg0` harness before delegating to `run_main` in the library crate.

## Detailed Behavior
- Invokes `arg0_dispatch_or_else` so the executable can run either in the sandbox (when invoked via `arg0`) or directly.
- Passes the path to the optional `codex-linux-sandbox` binary into `run_main`, along with default CLI configuration overrides.
- Returns `anyhow::Result<()>`, propagating any initialization failures back to the process.

## Broader Context
- Delegates all substantive behavior to `run_main` (`./lib.rs.spec.md`), keeping the binary thin.
- Shares the same arg0 dispatch pattern as other executables (e.g., `core::exec`, `cli`) for consistent sandbox startup semantics.

## Technical Debt
- None identified; main.rs intentionally remains a minimal delegate.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./lib.rs.spec.md
  - ../../exec/src/main.rs.spec.md
