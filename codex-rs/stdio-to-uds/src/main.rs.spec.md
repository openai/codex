## Overview
`main.rs` provides the CLI entrypoint for the stdio-to-UDS relay. It validates arguments, prints usage errors, and delegates to `codex_stdio_to_uds::run`.

## Detailed Behavior
- Expects exactly one argument (`<socket-path>`); prints usage and exits with status 1 on errors.
- Converts the argument to `PathBuf` and calls `run`, propagating any `anyhow::Error` to the caller.

## Broader Context
- Acts as a thin wrapper around the library so the tool can be invoked directly from the command line.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
