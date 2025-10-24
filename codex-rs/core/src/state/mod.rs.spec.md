## Overview
`core::state` organizes session- and turn-scoped state shared across the Codex runtime. It splits responsibilities into service dependencies, persistent session data, and per-turn metadata, re-exporting the key structs for use in `codex.rs`.

## Detailed Behavior
- Re-exports `SessionServices`, `SessionState`, `ActiveTurn`, `RunningTask`, and `TaskKind` from their respective submodules so other parts of the crate can import them from `crate::state`.
- Keeps the module graph tidy by hiding implementation details (e.g., `TurnState`) behind the public re-exports, ensuring only the necessary types leak into other modules.

## Broader Context
- Serves as the bridge between the orchestration layer (`codex.rs`) and the concrete state machinery defined in `service.rs`, `session.rs`, and `turn.rs`. Specs for those files expand on the data they carry.
- Context can't yet be determined for future grouping (e.g., splitting execution state vs. MCP state); this module will remain the aggregation point regardless of internal rearrangements.

## Technical Debt
- None observed; the module is strictly re-export glue.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./session.rs.spec.md
  - ./service.rs.spec.md
  - ./turn.rs.spec.md
  - ../codex.rs.spec.md
