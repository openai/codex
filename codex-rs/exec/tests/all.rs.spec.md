## Overview
`tests/all.rs` serves as the integration-test entrypoint for the exec crate, declaring the `suite` module so all scenarios compile into one binary.

## Detailed Behavior
- Contains `mod suite;` and no additional logic.

## Broader Context
- Mirrors the integration harness layout used in other crates.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.spec.md
  - ./suite/mod.rs.spec.md
