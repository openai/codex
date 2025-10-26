## Overview
`linux-sandbox/tests` groups integration tests for the sandbox binary. The suite is compiled into a single test binary via `tests/all.rs` and lives under `tests/suite`.

## Detailed Behavior
- `all.rs` aggregates the suite module.
- `suite/landlock.rs` validates sandbox permissions, writable roots, timeouts, and network restrictions on Linux hosts.

## Broader Context
- Mirrors the integration-test layout used across the workspace, ensuring sandbox regressions surface during CI.

## Technical Debt
- None; structure matches other crates’ test harnesses.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./all.rs.spec.md
  - ./suite/mod.rs.spec.md
  - ./suite/landlock.rs.spec.md
