## Overview
`tests/all.rs` is the integration test binary entrypoint. It simply includes the `suite` module so `cargo test` builds all login integration scenarios into one executable.

## Detailed Behavior
- Declares `mod suite;`, ensuring Rust’s integration-test harness compiles `tests/suite/*`.
- Matches the workspace pattern used across Codex for namespacing integration suites.

## Broader Context
- Keeps login integration tests discoverable while avoiding multiple binaries; this mirrors setups in other crates (e.g., app-server, core).

## Technical Debt
- None; file intentionally remains minimal.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.spec.md
  - ./suite/mod.rs.spec.md
