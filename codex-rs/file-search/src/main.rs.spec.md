## Overview
`main.rs` is the CLI entrypoint for `codex-file-search`. It parses arguments, configures stdout presentation, and drives the library’s async search routine.

## Detailed Behavior
- Parses `Cli` via Clap and constructs a `StdioReporter` with two toggles:
  - `write_output_as_json` mirrors `--json`.
  - `show_indices` requires both `--compute-indices` and an interactive stdout (`IsTerminal`) so ANSI highlighting is only used when the terminal can render it.
- Invokes `run_main` from the library, forwarding the parsed CLI and reporter; propagates any errors through `anyhow::Result`.
- `StdioReporter` implements `Reporter`:
  - `report_match` prints newline-delimited JSON, bold-highlighted paths, or plain paths based on the reporter configuration. Index highlighting iterates once through the pre-sorted index list for efficiency.
  - `warn_matches_truncated` either emits a JSON sentinel (`{"matches_truncated": true}`) or a human-readable warning.
  - `warn_no_search_pattern` logs the directory listing reminder mirrored by `run_main`’s fallback.

## Broader Context
- Provides the user-facing shell around the reusable search engine in `lib.rs`. Other binaries (e.g., MCP server) embed the library directly with custom reporters to integrate results into their own protocols.
- Shares reporting semantics with Codex tooling so CLI output stays predictable during support triage.

## Technical Debt
- None observed; reporter behavior aligns with current CLI requirements.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
