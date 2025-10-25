## Overview
`lib.rs` keeps the backend-client crate’s public surface minimal and cohesive. It wires up the internal modules and re-exports the main client and response helpers so downstream crates can import them from a single entrypoint.

## Detailed Behavior
- Declares the private `client` module and public `types` module.
- Re-exports:
  - `Client` – the HTTP wrapper around Codex/ChatGPT endpoints.
  - Typed response helpers (`CodeTaskDetailsResponse`, `CodeTaskDetailsResponseExt`, `PaginatedListTaskListItem`, `TaskListItem`, `TurnAttemptsSiblingTurnsResponse`) sourced from `types.rs`.
- Keeps the crate API stable even if module internals evolve, allowing consumers to `use codex_backend_client::Client` directly.

## Broader Context
- Consumed by Codex core orchestration, CLI, and app-server code when they need backend interactions.
- Bridges hand-rolled helpers with OpenAPI-generated models from `codex-backend-openapi-models`.

## Technical Debt
- None; the module is intentionally thin.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./client.rs.spec.md
  - ./types.rs.spec.md
