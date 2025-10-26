## Overview
`tests/all.rs` serves as the integration test entrypoint for the Linux sandbox suite, declaring the `suite` module so Cargo builds a single binary.

## Detailed Behavior
- Contains `mod suite;` and no additional logic.

## Broader Context
- Matches the project-wide convention for integration test aggregation.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.spec.md
  - ./suite/mod.rs.spec.md
