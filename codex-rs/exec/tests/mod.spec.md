## Overview
`exec/tests` hosts integration and unit tests for the Codex execution crate. The suite is organized under `tests/suite` with a single binary entrypoint (`tests/all.rs`), and includes standalone tests like `event_processor_with_json_output.rs`.

## Detailed Behavior
- `all.rs` aggregates the suite modules.
- `event_processor_with_json_output.rs` validates JSONL export of execution events.
- `suite` contains scenario-driven tests covering sandbox behavior, originator headers, apply-patch workflows, resumable sessions, auth env wiring, and more.

## Broader Context
- Ensures the execution pipeline remains compatible with CLI/tool expectations documented in Phase 1.

## Technical Debt
- Tests rely on a mix of mocks and real subprocesses; consolidating shared fixtures could improve maintainability, but the current structure matches workspace conventions.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Factor shared helpers (spawn wrappers, fixture builders) into reusable modules to reduce duplication across suite files.
related_specs:
  - ./all.rs.spec.md
  - ./event_processor_with_json_output.rs.spec.md
  - ./suite/mod.rs.spec.md
