## Overview
`core::openai_model_info` encodes baked-in metadata for OpenAI and Codex model families. It exposes `get_model_info`, which returns context window, output token limits, and default auto-compaction thresholds so the runtime can size prompts and decide when to compact history without hitting provider ceilings.

## Detailed Behavior
- `ModelInfo` stores three values per model:
  - `context_window`: maximum combined input tokens.
  - `max_output_tokens`: provider-advertised generation cap.
  - `auto_compact_token_limit`: defaults to 90% of the context window; callers can use this to trigger compaction ahead of hard failures.
- `MODEL_INFO::new` sets `auto_compact_token_limit` using the `default_auto_compact_limit` helper so every hard-coded entry maintains the same ratio.
- `get_model_info` matches on `ModelFamily::slug`:
  - Provides explicit entries for named models (`gpt-4.1`, `gpt-4o` snapshots, `o3`, `o4-mini`, `codex-mini-latest`, GPT-OSS variants, `gpt-3.5-turbo`), referencing OpenAI documentation comments for traceability.
  - Falls back to shared constants (`CONTEXT_WINDOW_272K`, `MAX_OUTPUT_TOKENS_128K`) for slug prefixes (`gpt-5`, `gpt-5-codex`, `codex-`), ensuring new snapshot IDs inherit sensible defaults without code changes.
  - Returns `None` for unknown slugs so callers can supply manual overrides (e.g., via configuration) or handle unsupported providers gracefully.

## Broader Context
- `ModelClient::get_model_context_window` and other client helpers call `get_model_info` to size prompts, compute auto-compaction thresholds, and warn users when model-specific features (verbosity, reasoning summaries) are available.
- Configuration surfaces may override these hard-coded values in TOML, but the defaults keep CLI/TUI behavior predictable even when users skip customization.
- Context can't yet be determined for pricing or rate-limit metadata; adding such fields would require configuration support and versioning across the plan.

## Technical Debt
- Model values and documentation references are manually curated; drifting OpenAI defaults require code updates. Moving these constants to structured data (JSON/TOML) or relying on provider responses would lower maintenance churn.
- Prefix-based fallbacks lump all `codex-` models into the same window/output limits; if future Codex models diverge, additional granularity will be needed.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Externalize model metadata (e.g., generated table or config file) so updates don’t require code changes when OpenAI revises limits.
    - Add explicit entries for new Codex-branded models once their token limits differ from the generic 272k/128k defaults.
related_specs:
  - ./client.rs.spec.md
  - ./client_common.rs.spec.md
  - ./model_provider_info.rs.spec.md
