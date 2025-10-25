## Overview
`cli.rs` defines the command-line interface for `codex cloud`. It uses Clap to expose both the default TUI mode and a subcommand for submitting tasks non-interactively.

## Detailed Behavior
- `Cli` derives `Parser`, bundling shared `CliConfigOverrides` (populated upstream) and an optional `Command`.
- `Command::Exec` triggers the `run_exec_command` path in `lib.rs`, accepting:
  - `query` (optional prompt or `-` to read from stdin),
  - `environment` id (required),
  - `attempts` best-of count (validated by `parse_attempts`, clamping to 1–4).
- `parse_attempts` enforces the numeric range and provides descriptive error messages for invalid input.

## Broader Context
- Parsed in `lib.rs::run_main`; when `command` is `None`, the TUI launches, otherwise the exec fast path runs.
- Shares the `CliConfigOverrides` pattern used across Codex CLI entrypoints for consistency, although the overrides are currently unused within this crate.

## Technical Debt
- None; the CLI surface is intentionally small pending future subcommands.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
