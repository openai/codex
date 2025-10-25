## Overview
`codex-async-utils` currently provides a single extension trait for awaiting futures with a cancellation token. It helps Codex tasks respect cooperative cancellation while preserving result types.

## Detailed Behavior
- `src/lib.rs` defines `CancelErr` and the `OrCancelExt` trait implemented for any `Future`.

## Broader Context
- Used wherever Codex tasks need to abort early when a `CancellationToken` fires.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
