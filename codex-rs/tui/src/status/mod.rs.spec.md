## Overview
`codex-tui::status` formats the `/status` command output. It assembles cards for account info, rate limits, and configuration details, and provides helpers for rendering rate-limit snapshots inline (e.g., in the chat widget).

## Detailed Behavior
- Submodules:
  - `account`: formats account metadata (plan type, workspace).
  - `card`: builds the overall status block (`new_status_output`).
  - `format`: shared formatting helpers (durations, table layouts).
  - `helpers`: reusable bits for status presentation.
  - `rate_limits`: renders `RateLimitSnapshotDisplay` and inline rate-limit usage, exposing `rate_limit_snapshot_display`.
- Exports:
  - `new_status_output` (main entrypoint to generate status text).
  - `RateLimitSnapshotDisplay` / `rate_limit_snapshot_display` for inline use in the bottom pane/status indicator.
- Tests live under `status/tests.rs` to validate formatting.

## Broader Context
- Chat widget invokes `new_status_output` when the user runs `/status`.
- Rate-limit display helpers are reused to show warnings in the composer/status indicator.
- Context can't yet be determined for future status sections; the modular structure allows new cards to be added to `card.rs`.

## Technical Debt
- None noted; module decomposition is clean.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./chatwidget.rs.spec.md
  - ./bottom_pane/mod.rs.spec.md
