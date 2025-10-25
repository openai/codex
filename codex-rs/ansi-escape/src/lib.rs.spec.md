## Overview
`lib.rs` converts ANSI-colored strings into ratatui text structures. It wraps `ansi-to-tui`, normalizes tabs for better gutter alignment, and offers both multi-line (`ansi_escape`) and single-line (`ansi_escape_line`) variants with detailed error logging.

## Detailed Behavior
- `expand_tabs` replaces tab characters with four spaces when present, returning a `Cow<str>` so the caller can borrow untouched strings.
- `ansi_escape_line`:
  - Normalizes tabs, converts the text via `ansi_escape`, and expects a single line.
  - Logs a warning if multiple lines are produced, returning the first line.
- `ansi_escape` uses `IntoText` from `ansi-to-tui`, panicking with detailed logs if the parser returns `NomError` (unexpected) or `Utf8Error`.
- The helper ensures transcripts render consistently in the TUI/CLI even when tools emit ANSI codes or tab-delimited output.

## Broader Context
- Used by transcript rendering in Codex’s TUI and CLI to preserve color formatting while integrating with ratatui widgets.

## Technical Debt
- None; the simplistic tab handling is sufficient for current use cases.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
