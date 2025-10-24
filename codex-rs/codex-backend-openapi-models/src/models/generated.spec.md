## Overview
The files under `codex-backend-openapi-models/src/models/*.rs` (excluding `mod.rs`) are generated from the Codex backend OpenAPI definition. They mirror the backend’s JSON schemas so Rust clients can serialize and deserialize responses without manual glue code.

## Detailed Behavior
- Generation pulls the latest OpenAPI spec and runs the internal generator script (documented in team runbooks). The process overwrites existing model files and may introduce new modules when the backend adds endpoints.
- Generated structs derive serde traits and typically include builders or utility methods emitted by the generator. Some files add manual adjustments afterward (e.g., renaming fields) but should remain minimal to keep regeneration painless.
- Because generated code routinely contains `unwrap` or `expect` calls, the crate-level `#![allow]` in `lib.rs` suppresses Clippy warnings.

### Regeneration Checklist
1. Fetch the latest OpenAPI schema from the backend API repository.
2. Run the internal generator (see shared script reference) targeting `src/models/`.
3. Review diffs for breaking changes; update `src/models/mod.rs` to export any new types.
4. Run `cargo fmt` within the crate.
5. Execute the dependent crates’ test suites (e.g., `backend-client`) to confirm the new models deserialize as expected.

## Broader Context
- Downstream crates assume these models stay synchronized with the backend schema. Regenerating should coincide with backend deployments to keep the client contract current.
- When the backend deprecates endpoints, stale generated files should be removed and the exports pruned accordingly.
- Context can't yet be determined for versioned schemas; future work may require namespacing these generated files by API version.

## Technical Debt
- Regeneration remains a manual process; codifying the generator invocation in the repository (e.g., via a script or `just` recipe) would streamline updates and improve reproducibility.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Check in a documented generator command or script path so contributors know how to refresh the models without tribal knowledge.
related_specs:
  - ../../mod.spec.md
  - ../lib.rs.spec.md
  - ./mod.rs.spec.md
