## Overview
`tests::suite::mod` aggregates the exec integration scenarios covering sandbox enforcement, auth environment wiring, resume flows, apply-patch behavior, and error handling.

## Detailed Behavior
- Declares modules for `apply_patch`, `auth_env`, `originator`, `output_schema`, `resume`, `sandbox`, and `server_error_exit`, bringing each scenario into the test binary.

## Broader Context
- Follows the same suite pattern used across the workspace, keeping the integration entrypoint declarative.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./apply_patch.rs.spec.md
  - ./auth_env.rs.spec.md
  - ./originator.rs.spec.md
  - ./output_schema.rs.spec.md
  - ./resume.rs.spec.md
  - ./sandbox.rs.spec.md
  - ./server_error_exit.rs.spec.md
