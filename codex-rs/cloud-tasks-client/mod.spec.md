## Overview
`codex-cloud-tasks-client` abstracts Codex Cloud backends. It defines the shared data model (`api.rs`) and provides two implementations of the `CloudBackend` trait: a real HTTP client (feature `online`) and a mock client (feature `mock`) for tests or offline demos.

## Detailed Behavior
- `src/lib.rs` exports the public API: task/apply models, the `CloudBackend` trait, and, when enabled, the `HttpClient` and `MockClient` concrete implementations.
- `src/api.rs` defines error types, task/attempt/result structs, and the async `CloudBackend` interface used by higher-level clients (TUI, CLI).
- `src/http.rs` implements the live backend integration using `codex-backend-client` and the `codex-git-apply` engine.
- `src/mock.rs` supplies static responses for development and testing under the `mock` feature flag.

## Broader Context
- The crate is consumed by `codex-cloud-tasks` (UI/CLI) and potentially other automation tooling. By gating implementations behind features, builds can swap between real network access and deterministic mocks.

## Technical Debt
- None noted; open issues (e.g., additional backend endpoints) will add new trait methods rather than adjustments here.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/api.rs.spec.md
  - ./src/http.rs.spec.md
  - ./src/mock.rs.spec.md
