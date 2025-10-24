## Overview
`core::tools::handlers::grep_files` wraps the `rg` (ripgrep) CLI to locate files containing a given pattern. It enforces timeouts, validates inputs, and trims results to a configurable limit before returning them to the model.

## Detailed Behavior
- Accepts `ToolPayload::Function` parsed into `GrepFilesArgs`, requiring a non-empty `pattern`, optional `include` glob, optional search `path` (resolved against the turn’s cwd), and a positive `limit` (capped at 2000).
- Verifies the search path exists via `tokio::fs::metadata` and normalizes empty strings to `None`.
- `run_rg_search` constructs an `rg --files-with-matches --sortr=modified` command, adds the pattern and optional glob filter, and executes it within the configured cwd. Execution is wrapped in a 30-second timeout; timeouts and spawn failures are reported back to the model with actionable messages (e.g., “ensure ripgrep is installed”).
- Exit code handling:
  - `0`: parse stdout into a list of file paths (respecting the limit).
  - `1`: treat as “no matches” and return an empty list.
  - Other codes: return stderr as an error.
- When matches are found, joins them with newlines and returns `ToolOutput::Function { success: Some(true) }`. Empty results emit “No matches found.” with `success = Some(false)` so callers can distinguish between empty and errored searches.

## Broader Context
- The tool is experimental and registered only when the model family supports it. It complements `read_file` and `list_dir` by helping the model locate files before reading them.
- Using ripgrep keeps behavior consistent with CLI tooling already bundled in developer environments; the handler surfaces explicit errors when the binary is missing or times out.
- Context can't yet be determined for streaming large result sets; current behavior truncates to the limit for simplicity.

## Technical Debt
- None noted; future enhancements could expose additional ripgrep flags if needed.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.rs.spec.md
  - ../spec.rs.spec.md
  - ../../mod.spec.md
