## Overview
`mock.rs` provides a lightweight in-memory `CloudBackend` implementation for tests and offline demos. It returns deterministic task lists, diffs, messages, and apply outcomes without hitting the real backend.

## Detailed Behavior
- `MockClient` implements `CloudBackend` with canned responses:
  - `list_tasks` returns environment-specific task sets (differentiating by env id) and populates diff summaries using helper functions.
  - `get_task_diff`, `get_task_messages`, and `get_task_text` supply predictable content for UI rendering.
  - `list_sibling_attempts` returns an alternate attempt for a specific task id (`T-1000`) and empty otherwise.
  - `apply_task` and `apply_task_preflight` always succeed, returning friendly messages.
  - `create_task` synthesizes a timestamp-based task id.
- Helpers `mock_diff_for`, `count_from_unified` generate mock diff payloads and summary counts. Fallback diff counting handles non-unified output to keep mock resilient.

## Broader Context
- Enabled via the `mock` feature and used by the Cloud Tasks TUI during development or in unit tests (e.g., verifying state transitions without network calls).
- Shares struct definitions from `api.rs` so the rest of the application doesn’t need conditional code paths.

## Technical Debt
- None; any additional backend methods should extend this mock to keep test coverage aligned.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./api.rs.spec.md
