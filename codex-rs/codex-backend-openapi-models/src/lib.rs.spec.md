## Overview
`codex-backend-openapi-models::lib` is a thin wrapper that re-exports the generated OpenAPI models. It keeps the crate free of hand-written types while providing a stable import path for consumers.

## Detailed Behavior
- Applies `#![allow(clippy::unwrap_used, clippy::expect_used)]` at the crate root to acknowledge patterns commonly emitted by the OpenAPI generator.
- Declares the `models` module, which is populated by the generator and curated via `src/models/mod.rs`. No additional logic or re-exports live in this file.

## Broader Context
- Serves as the canonical entry point for other crates: clients import types via `codex_backend_openapi_models::models::TaskResponse`, avoiding direct references to generated filenames.
- Any regeneration that adds new modules only needs to update `src/models/mod.rs`; this file remains untouched unless lints or module paths change.
- Context can't yet be determined for namespacing multiple OpenAPI versions; if the backend publishes versioned schemas, this module will need to coordinate separate namespaces.

## Technical Debt
- None observed; the module intentionally stays minimal.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./models/mod.rs.spec.md
  - ./models/generated.spec.md
