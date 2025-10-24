## Overview
`codex-backend-openapi-models::models::mod` curates the public exports from the generated OpenAPI types. It re-exports only the structs currently consumed by the workspace so the crate does not expose the full (and frequently changing) backend schema surface.

## Detailed Behavior
- Declares submodules for each generated file under `src/models/` and re-exports their primary types using `pub use`. The current export set covers task metadata, external pull requests, and rate-limit snapshots used by `backend-client` and related crates.
- Comments at the top note that the file used to be auto-generated; maintainers now manually prune the list after running the OpenAPI generator. This avoids churn from unused types and keeps downstream dependency graphs tidy.
- The curated exports group related types (cloud tasks vs. rate limits) to make future updates easier to scan.

## Broader Context
- When regenerating models, contributors must review the new files in `src/models/` and update this module to expose any newly required types. Failing to do so can result in compile errors in dependent crates.
- As additional backend endpoints migrate to Codex clients, this module provides the single place to audit which API shapes have been adopted.
- Context can't yet be determined for automating export reduction; until then, this file represents the agreed contract with downstream consumers.

## Technical Debt
- Manual export curation can easily drift from actual usage; a follow-up automation or linter would help ensure no required types are omitted.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Add tooling to validate that types imported by dependent crates are exported here, especially after schema regeneration.
related_specs:
  - ../../mod.spec.md
  - ../lib.rs.spec.md
  - ./generated.spec.md
