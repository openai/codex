## Overview
`core::features::legacy` bridges older configuration keys with the new feature system. It maps historical boolean toggles to modern `Feature` variants and logs deprecation warnings so users migrate to the `[features]` table.

## Detailed Behavior
- `ALIASES` lists legacy keys (e.g., `experimental_use_unified_exec_tool`, `include_apply_patch_tool`, `tools.web_search`) and maps them to `Feature` variants.
- `feature_for_key` resolves legacy entries when parsing `[features]` maps, logging a hint that `[features].<canonical>` should be used instead.
- `LegacyFeatureToggles` aggregates legacy boolean fields drawn from `ConfigToml`, profiles, or CLI overrides. `apply` iterates through each toggle, enabling/disabling the corresponding feature via `set_if_some`.
- Logging (`log_alias`) records when a legacy key is used, aiding telemetry and user guidance.

## Broader Context
- `Features::from_config` constructs `LegacyFeatureToggles` from base config, profile, and overrides before applying modern `[features]` tables. This ensures behaviour remains stable while users transition to the canonical keys.
- Keeping legacy logic isolated simplifies future removal once old keys are phased out.

## Technical Debt
- None noted; once adoption of `[features]` is complete, this module can be retired cleanly.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../features.rs.spec.md
