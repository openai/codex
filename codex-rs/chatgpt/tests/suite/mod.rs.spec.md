## Overview
`tests::suite::mod` groups the chatgpt integration scenarios.

## Detailed Behavior
- Declares `mod apply_command_e2e;`, pulling the end-to-end apply command tests into the binary.

## Broader Context
- Keeps the integration structure consistent with other crates.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./apply_command_e2e.rs.spec.md
