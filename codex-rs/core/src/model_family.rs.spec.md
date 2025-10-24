## Overview
`core::model_family` categorizes model slugs into families and records per-family capabilities used across Codex. It governs instruction templates, tool expectations, reasoning support, and context-window adjustments so that downstream clients can tailor payloads without hard-coding individual models.

## Detailed Behavior
- `ModelFamily` captures:
  - Identifiers (`slug`, `family`) for grouping snapshot variants.
  - Instruction flags (`needs_special_apply_patch_instructions`, `base_instructions`, `uses_local_shell_tool`, `apply_patch_tool_type`).
  - Reasoning knobs (`supports_reasoning_summaries`, `reasoning_summary_format`, `supports_parallel_tool_calls`).
  - Experimental tool list and effective context-window percentage used by `ModelClient`.
- The `model_family!` macro initializes a struct with defaults (base prompt, no special tools, 95 % usable context window) and applies overrides for each family.
- `find_family_for_model` inspects the slug prefix to return an appropriate family configuration:
  - Public OpenAI families (`o3`, `o4-mini`, `gpt-4.1`, `gpt-4o`, `gpt-3.5`) enable apply-patch instructions and reasoning as needed.
  - OSS and internal Codex models set local-shell support, experimental tool lists, and custom prompts (`GPT_5_CODEX_INSTRUCTIONS`).
  - `test-gpt-5-codex` mirrors Codex production config while enabling a wider experimental toolset for validation.
- `derive_default_model_family` provides a fallback with baseline instructions and no special capabilities when the slug is unknown.

## Broader Context
- Prompt construction (`client_common::Prompt`, `client.rs`) uses these flags to decide instruction text, reasoner parameters, and whether to expose local shell/apply-patch tools.
- Context-window heuristics combine with `openai_model_info` to compute effective limits; telemetry and compaction strategies depend on consistent family metadata.
- Context can't yet be determined for future model features (e.g., JSON schema enforcement differences); new flags should be added here to propagate through the client stack.

## Technical Debt
- slug-prefix matching is brittle; as OpenAI/Codex release more snapshot variants the cascade must be kept in sync. A data-driven configuration (TOML/YAML) would simplify updates and allow workspace overrides.
- `derive_default_model_family` duplicates the struct defaults; consolidating default construction in one place would avoid divergence when new fields are added.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Externalize model-family definitions into a configuration table so new snapshots can be added without editing code.
    - Refactor default initialization to avoid duplicating field assignments between the macro and `derive_default_model_family`.
related_specs:
  - ./client_common.rs.spec.md
  - ./client.rs.spec.md
  - ./openai_model_info.rs.spec.md
