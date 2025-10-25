## Overview
`codex-ansi-escape` converts ANSI-colored strings into ratatui `Text`/`Line` values for Codex transcript rendering, normalizing tab characters to avoid gutter issues. It wraps the `ansi-to-tui` crate and exposes helpers for full text or single-line use cases.

## Detailed Behavior
- `src/lib.rs` provides `ansi_escape` and `ansi_escape_line`, plus a tab-expansion helper.

## Broader Context
- Used by the TUI and CLI transcript views to render ANSI output from tools and model responses cleanly.

## Technical Debt
- None noted.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
