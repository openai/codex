## Overview
`lib.rs` exposes the public surface of the cloud-tasks-client crate. It re-exports the data models and the `CloudBackend` trait, and conditionally publishes the concrete implementations for mock and HTTP backends based on Cargo features.

## Detailed Behavior
- Declares the internal `api` module and re-exports:
  - `ApplyOutcome`, `ApplyStatus`, `AttemptStatus`, `CloudBackend`, `CloudTaskError`, `CreatedTask`, `DiffSummary`, `Result`, `TaskId`, `TaskStatus`, `TaskSummary`, `TaskText`, `TurnAttempt`.
- Conditionally compiles:
  - `mock` module (`MockClient`) behind the `mock` feature.
  - `http` module (`HttpClient`) behind the `online` feature.
- Provides a single place for downstream crates to import types regardless of which backend implementation is enabled at build time.

## Broader Context
- Used by `codex-cloud-tasks` and other clients to avoid depending on implementation details; build profiles decide whether to link the HTTP backend, the mock backend, or both.

## Technical Debt
- None; module gating cleanly separates implementation choices.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./api.rs.spec.md
  - ./http.rs.spec.md
  - ./mock.rs.spec.md
