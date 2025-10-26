## Overview
`apply-patch/tests` contains integration coverage for the standalone `apply_patch` binary, compiled via `tests/all.rs` to exercise CLI behaviors end to end.

## Detailed Behavior
- `all.rs` aggregates the `suite` module, which currently focuses on CLI scenarios in `suite/cli.rs`.

## Broader Context
- Ensures the standalone binary behaves consistently with patch semantics documented for the core tooling.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./all.rs.spec.md
  - ./suite/mod.rs.spec.md
  - ./suite/cli.rs.spec.md
