## Overview
`tui::main` is the binary entrypoint for the Codex TUI. It wires together top-level CLI arguments, arg0 dispatch (for Linux sandbox passthrough), and the async `run_main` loop exported by the library crate.

## Detailed Behavior
- Uses `clap::Parser` to define a `TopCli` struct that combines shared `CliConfigOverrides` with the TUI-specific `Cli`.
- Invokes `arg0_dispatch_or_else` to support `codex arg0` execution on Linux, passing the resolved sandbox binary path to the async closure.
- Parses CLI arguments, merges global config overrides ahead of the inner TUI overrides, and calls `run_main`.
- On successful exit, prints token usage as a `FinalOutput` if the session consumed tokens, mirroring CLI defaults.

## Broader Context
- This binary is what `cargo install codex-tui` produces; the reusable application logic lives in `src/lib.rs` and `src/tui.rs`.

## Technical Debt
- None; entrypoint is intentionally thin and defers to library code.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./cli.rs.spec.md
  - ./tui.rs.spec.md
