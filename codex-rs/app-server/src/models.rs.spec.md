## Overview
`models` exposes the list of models the app server advertises to clients. It lifts preset metadata from `codex-common` and converts it into the JSON-RPC wire schema.

## Detailed Behavior
- `supported_models` pulls the full list of builtin model presets (`builtin_model_presets(None)`), maps each preset through `model_from_preset`, and returns `Vec<Model>` ready for RPC responses.
- `model_from_preset` copies identifiers, descriptions, default flags, and reasoning effort configuration into the protocol’s `Model` struct.
- `reasoning_efforts_from_preset` converts each `ReasoningEffortPreset` into a `ReasoningEffortOption` with human-readable descriptions.

## Broader Context
- Used by `CodexMessageProcessor::list_models` (`./codex_message_processor.rs.spec.md`) to populate the model selector in the VS Code client.
- Keeps the app server aligned with shared presets defined in `codex-common`, so changes to presets automatically propagate to clients without duplicating data.

## Technical Debt
- None; the module simply adapts shared presets for RPC output.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./codex_message_processor.rs.spec.md
  - ../../common/src/model_presets.rs.spec.md
