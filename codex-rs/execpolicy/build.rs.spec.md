## Overview
`build.rs` for `codex-execpolicy` instructs Cargo to rerun the build script whenever `src/default.policy` changes, ensuring policy updates rebuild the crate.

## Detailed Behavior
- Emits `cargo:rerun-if-changed=src/default.policy` during build.

## Broader Context
- Keeps the compiled policy in sync with source updates; downstream crates rely on this policy during sandbox execution.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.spec.md
