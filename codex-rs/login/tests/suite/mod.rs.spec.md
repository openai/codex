## Overview
`tests::suite::mod` groups the login integration modules, keeping the main `tests/all.rs` file declarative.

## Detailed Behavior
- Declares `mod device_code_login;` and `mod login_server_e2e;`, bringing both scenarios into the compiled binary.

## Broader Context
- Mirrors the suite module layout used in other crates, enabling editors and tooling to locate integration scenarios quickly.

## Technical Debt
- None; file intentionally stays minimal.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./device_code_login.rs.spec.md
  - ./login_server_e2e.rs.spec.md
