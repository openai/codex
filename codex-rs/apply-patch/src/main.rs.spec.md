## Overview
`main.rs` is the minimal binary entrypoint for the `apply_patch` tool. It delegates directly to the library’s standalone executable wrapper.

## Detailed Behavior
- Calls `codex_apply_patch::main()` (re-exported from `standalone_executable.rs`), ensuring the binary and library stay in sync without duplicating code.

## Broader Context
- Provides the published CLI for developers; all substantive logic lives in `standalone_executable.rs`.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./standalone_executable.rs.spec.md
