## Overview
`core::tools::handlers` declares the concrete tool handlers that implement `ToolHandler` for each registered tool. It re-exports handler types so the registry and spec builder can register them without referencing deep module paths.

## Detailed Behavior
- Organizes handlers into submodules (`shell`, `apply_patch`, `unified_exec`, `mcp`, etc.) and publicly re-exports the handler structs.
- Exposes `PLAN_TOOL`, the pre-built `ToolSpec` for the plan/update tool, which is reused by `spec.rs` when constructing the tool list.
- The actual handler logic lives in the submodules; this module serves as the wiring layer to consolidate exports and avoid scattered public module declarations.

## Broader Context
- `spec.rs` imports these handler types to register them with the `ToolRegistryBuilder`. Keeping exports centralized makes it easy to audit which handlers exist and ensures new handlers are visible to the builder.
- Context can't yet be determined for feature-gated handlers; currently, all handlers are compiled in, with inclusion controlled by `spec.rs`.

## Technical Debt
- None observed; the module is purely structural.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../spec.rs.spec.md
  - ../registry.rs.spec.md
