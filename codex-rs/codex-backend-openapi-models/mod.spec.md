## Overview
`codex-backend-openapi-models` surfaces the subset of backend OpenAPI models that Codex services consume. The crate primarily hosts generated structs that mirror the Codex cloud API and re-exports them for use in Rust clients such as `backend-client` and `cloud-tasks`.

## Detailed Behavior
- `src/lib.rs` re-exports the `models` module, which contains the generated OpenAPI types and a curated `mod.rs` that limits the public surface to the structs currently needed in the workspace.
- Regeneration pulls schema definitions from the internal OpenAPI generator (see `Generated Models` spec) and overwrites files in `src/models/`, after which `mod.rs` may require manual curation to adjust the exported list.
- The crate disables select Clippy lints in `Cargo.toml` and at the module level to accommodate generated code that may not follow workspace style rules.

## Broader Context
- Downstream crates rely on these models to deserialize responses from the Codex backend without manually maintaining JSON structures. Regenerating the models should be part of any backend API version update.
- Because only a subset of the generated types are exported, teams adding new API endpoints must update `src/models/mod.rs` to re-export the additional structs.
- Context can't yet be determined for an automated pruning workflow; for now, the curated export list is maintained by hand.

## Technical Debt
- Manual curation of `src/models/mod.rs` is error-prone when new schemas are introduced; an automated or documented checklist would reduce risk.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Automate or script the pruning of `src/models/mod.rs` after regeneration to avoid missing required exports.
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/models/mod.rs.spec.md
  - ./src/models/generated.spec.md
