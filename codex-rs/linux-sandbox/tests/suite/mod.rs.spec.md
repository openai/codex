## Overview
`tests::suite::mod` gathers the Linux sandbox integration modules.

## Detailed Behavior
- Declares `mod landlock;` so the `landlock` tests compile into the suite.

## Broader Context
- Keeps the integration binary layout consistent with other Codex crates.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./landlock.rs.spec.md
