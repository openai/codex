## Overview
`codex-feedback` captures recent log output in-memory so users can upload Codex session logs to Sentry or save them locally. It exposes `CodexFeedback` for log buffering, a `MakeWriter` implementation for tracing, and helpers to snapshot and ship logs.

## Detailed Behavior
- `src/lib.rs` implements the ring buffer, writer adapters, and upload utilities.

## Broader Context
- Used by CLI/TUI features that collect logs when users submit feedback or bug reports.

## Technical Debt
- None noted.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
