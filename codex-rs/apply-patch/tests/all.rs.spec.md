## Overview
`tests/all.rs` builds the apply-patch integration binary by including the `suite` module.

## Detailed Behavior
- Contains `mod suite;` only, following the workspace’s integration-test convention.

## Broader Context
- Keeps CLI test discovery aligned with other crates.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.spec.md
  - ./suite/mod.rs.spec.md
