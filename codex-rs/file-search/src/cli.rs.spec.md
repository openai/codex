## Overview
`cli.rs` defines the Clap-powered interface for the `codex-file-search` binary and any embedded callers that parse argv on its behalf.

## Detailed Behavior
- `Cli` derives `Parser`, exposing:
  - `--json` to emit newline-delimited JSON `FileMatch` values.
  - `--limit/-l` (default 64) as a `NonZero<usize>` to bound result counts.
  - `--cwd/-C` to override the search root.
  - `--compute-indices` to request highlight indices alongside results.
  - `--threads` (default 2) to control parallel walkers; the doc comment explains why the default is conservative.
  - `--exclude/-e` repeats to add ignore override patterns.
  - A positional `pattern` string that, when omitted, triggers the `ls` fallback described in `run_main`.
- struct fields are public so they can be consumed directly by `run_main` and other components without additional mapping.

## Broader Context
- Parsed in `main.rs` and re-exported through `lib.rs`, giving both the binary and library users a consistent argument surface.
- Options map cleanly onto the `run_main` inputs (`limit`, `cwd`, `threads`, `compute_indices`, `exclude`, `pattern`), ensuring no transformation loss.

## Technical Debt
- None; the CLI is narrowly scoped and delegates all normalization to `run_main`.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
