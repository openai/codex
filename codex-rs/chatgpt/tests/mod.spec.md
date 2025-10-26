## Overview
`chatgpt/tests` contains integration coverage for the chatgpt crate. The suite is compiled via `tests/all.rs` and currently focuses on end-to-end diff application.

## Detailed Behavior
- `all.rs` pulls in `tests/suite`, which houses `apply_command_e2e`.

## Broader Context
- Ensures the Phase 4 chatgpt specs are backed by integration tests that exercise Git patch application workflows.

## Technical Debt
- None; structure mirrors other crate test harnesses.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./all.rs.spec.md
  - ./suite/mod.rs.spec.md
  - ./suite/apply_command_e2e.rs.spec.md
