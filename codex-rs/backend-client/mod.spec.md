## Overview
`codex-backend-client` wraps Codex and ChatGPT backend REST APIs, providing a typed HTTP client plus data models for task metadata and rate-limit queries. The crate re-exports the client and enriched response helpers so other services can pull in backend functionality without depending directly on generated OpenAPI structs.

## Detailed Behavior
- `src/lib.rs` exposes the primary modules (`client`, `types`) and re-exports the `Client` plus convenience response types (`CodeTaskDetailsResponse`, `CodeTaskDetailsResponseExt`, `PaginatedListTaskListItem`, `TaskListItem`, `TurnAttemptsSiblingTurnsResponse`).
- `src/client.rs` implements the HTTP client, including base-url normalization, header construction, and helpers for rate limits, task listing, task detail retrieval, sibling turn enumeration, and task creation.
- `src/types.rs` wraps and extends OpenAPI-generated models with ergonomic accessors for task details, diffs, worklogs, and error messages.

## Broader Context
- Used by Codex core services and app-server integrations to talk to the Codex backend and ChatGPT “WHAM” endpoints; complements `codex-core::auth` to supply bearer tokens/user agents.
- Works alongside generated OpenAPI models (`codex-backend-openapi-models`) but fills gaps where custom traversal of response payloads is required.

## Technical Debt
- None noted; higher-level TODOs (e.g., hand-rolling all backend models) live in consuming services rather than this crate.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/client.rs.spec.md
  - ./src/types.rs.spec.md
