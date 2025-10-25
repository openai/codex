## Overview
`api.rs` defines the shared data model and `CloudBackend` trait for Codex Cloud integrations. It standardizes task metadata, apply workflows, and error handling so both HTTP and mock clients present the same interface.

## Detailed Behavior
- `CloudTaskError` enumerates failure modes (`Unimplemented`, `Http`, `Io`, generic `Msg`), and `Result<T>` aliases `std::result::Result<T, CloudTaskError>`.
- Core types:
  - `TaskId` newtype ensures consistent formatting.
  - `TaskStatus`, `TaskSummary`, and `DiffSummary` describe task overviews (status, labels, diff statistics, attempt counts).
  - `ApplyStatus` and `ApplyOutcome` capture apply/preflight results plus skipped/conflict path lists.
  - `CreatedTask` wraps task creation responses.
  - `AttemptStatus`, `TurnAttempt`, and `TaskText` model sibling attempts and assistant messages/prompts.
- `CloudBackend` trait specifies async methods for listing tasks, fetching diffs/messages, retrieving structured text, enumerating sibling attempts, running applies/preflights (with diff overrides), and creating tasks with optional best-of settings.
- `TaskText` and `AttemptStatus` default implementations make it easier to initialize optional fields.

## Broader Context
- Consumed by the HTTP implementation (`src/http.rs`) to translate backend API responses into these structs, and by the mock implementation for synthetic data.
- Higher-level clients rely on these definitions to keep business logic independent of the transport layer.

## Technical Debt
- None; the model is designed to evolve alongside the backend API by adding fields or trait methods as new endpoints appear.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./http.rs.spec.md
  - ./mock.rs.spec.md
