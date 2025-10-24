## Overview
`core::tools::handlers::apply_patch` adapts the `apply_patch` tool into the shared tool runtime pipeline. It accepts both freeform grammar and JSON/function payloads, verifies arguments, and delegates execution to `handle_container_exec_with_params` so patches run through the unified shell/apply-patch orchestrator.

## Detailed Behavior
- Accepts `ToolPayload::Function` (JSON arguments shaped by `ApplyPatchToolArgs`) and `ToolPayload::Custom` (freeform patch body). Any other payload emits a model-facing error.
- Builds `ExecParams` targeting the `apply_patch` CLI, passing the raw patch input as the second argument, keeping the current turn’s working directory, clearing environment variables, and leaving escalation unset.
- Calls `handle_container_exec_with_params`, which routes the request through approval checks, sandbox selection, diff tracking, and apply_patch runtime execution. The returned formatted output is wrapped in `ToolOutput::Function` with `success = Some(true)`.
- Provides helper constructors:
  - `create_apply_patch_freeform_tool` returns a freeform grammar tool spec using the `tool_apply_patch.lark` definition.
  - `create_apply_patch_json_tool` exposes the legacy JSON schema variant, used by model families that prefer function-calling payloads.

## Broader Context
- `spec.rs` decides which variant to expose based on model family capabilities and feature flags, registering this handler for both forms. The handler’s reliance on `handle_container_exec_with_params` keeps approval/sandbox logic consistent with shell execution.
- Diff tracking and event emission occur upstream; this handler only prepares the request.
- Context can't yet be determined for deprecating the JSON tool; once the freeform tool covers all models, the JSON helper can be removed alongside the corresponding spec toggle.

## Technical Debt
- None noted in this module; deprecation of the JSON variant is tracked in `tools/spec.rs`.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.rs.spec.md
  - ../spec.rs.spec.md
  - ../../mod.spec.md
