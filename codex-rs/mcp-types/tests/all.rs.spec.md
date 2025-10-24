## Overview
`mcp-types/tests/all.rs` defines the single integration test binary executed by `cargo test`. It simply exposes the `suite` module so individual test cases under `tests/suite` compile into one binary.

## Detailed Behavior
- Declares `mod suite;`, which pulls in every test module listed in `tests/suite/mod.rs`.
- Contains no logic beyond the module declaration; Rust’s integration test harness discovers and runs the actual functions within the imported modules.

## Broader Context
- This file preserves the older layout where each test lived in its own binary by aggregating them into a shared module. Adding new integration tests only requires creating a module under `tests/suite` and exposing it from `tests/suite/mod.rs`.
- Context can't yet be determined for splitting tests back into multiple binaries; the current layout favors faster compilation by reusing one crate.

## Technical Debt
- None observed; the file is intentionally minimal.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ../src/lib.rs.spec.md
