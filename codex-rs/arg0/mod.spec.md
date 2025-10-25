## Overview
`codex-arg0` implements the “arg0 trick” for Codex binaries. It inspects the invoked executable name to dispatch to specialized CLIs (sandbox, apply_patch), loads `~/.codex/.env`, adjusts `PATH`, and then runs the main async entrypoint with the appropriate sandbox executable path.

## Detailed Behavior
- `src/lib.rs` contains the dispatch logic, dotenv loading, and apply-patch path setup.

## Broader Context
- Used by Codex CLI binaries to simulate multiple executables while shipping a single binary.

## Technical Debt
- None noted.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
