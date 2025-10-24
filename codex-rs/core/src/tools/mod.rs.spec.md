## Overview
`core::tools` coordinates tool invocation for Codex turns. It exposes the router, registry, orchestrator, runtimes, sandboxing utilities, and telemetry helpers that allow model-generated tool calls to execute safely while surfacing consistent events and outputs.

## Detailed Behavior
- Re-exports submodules for context (`context`), event emission (`events`), routing (`router`), orchestration (`orchestrator`, `parallel`), execution backends (`runtimes`), sandbox enforcement (`sandboxing`), tool specifications (`spec`), and the handler registry (`registry`/`handlers`).
- Defines constants controlling truncation for model-facing summaries and telemetry previews (`MODEL_FORMAT_*`, `TELEMETRY_PREVIEW_*`), reused throughout tool output formatting.
- `handle_container_exec_with_params` is the primary entry point for shell/apply-patch tool calls routed through the new orchestrator. It:
  - Validates escalated-permission requests against the approval policy.
  - Detects `apply_patch` invocations via `maybe_parse_apply_patch_verified` and either applies patches inline or delegates to exec when verification fails.
  - Emits begin/success/failure events using `ToolEmitter`, wiring telemetry and diff tracking context (`SharedTurnDiffTracker`).
  - Constructs runtime-specific requests (`ApplyPatchRequest`, `ShellRequest`) and runs them through `ToolOrchestrator` with the appropriate runtime.
  - Normalizes outcomes via `handle_exec_outcome`, mapping successful exec outputs or errors into `FunctionCallError` responses for the model.
- Output formatting helpers (`format_exec_output_for_model`, `format_exec_output_str`, `format_exec_output`, `truncate_formatted_exec_output`) apply byte/line budgets, add truncation notices, and embed metadata (exit code, duration) before returning JSON payloads to the model. These helpers are also reused when summarizing telemetry failures.

## Broader Context
- `codex.rs` relies on this module to translate tool requests into deterministic behavior. Specs for the router, registry, sandboxing, and runtimes explain the finer-grained responsibilities.
- The truncation constants align with model prompt limits and telemetry budgets; any changes to model context handling should adjust these values in coordination with UI expectations.
- Context can't yet be determined for dynamic tool registration; TODOs in `registry.rs` hint at future support, which will require extending this module’s surfaces.

## Technical Debt
- `handle_container_exec_with_params` remains large and mixed with output formatting. TODOs indicate a future refactor to break it into smaller pieces; documenting the staging plan would help mitigate complexity.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Split `handle_container_exec_with_params` into composable helpers (parsing, orchestration, response shaping) to improve readability and testability.
related_specs:
  - ./router.rs.spec.md
  - ./registry.rs.spec.md
  - ./context.rs.spec.md
  - ./sandboxing.rs.spec.md
  - ../turn_diff_tracker.rs.spec.md
