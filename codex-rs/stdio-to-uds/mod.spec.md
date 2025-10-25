## Overview
`codex-stdio-to-uds` connects stdin/stdout to a Unix Domain Socket (UDS), allowing Codex tools to communicate with services exposed over UDS endpoints.

## Detailed Behavior
- `src/lib.rs` performs the relay; `src/main.rs` provides the CLI wrapper.

## Broader Context
- Useful for bridging local processes with sandboxed services or other components that prefer UDS communication.

## Technical Debt
- None noted.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/main.rs.spec.md
