## Overview
`core::config_profile` defines the subset of configuration that can be grouped into named profiles. Profiles allow users to toggle between different model/providers and feature presets without duplicating entire config files.

## Detailed Behavior
- `ConfigProfile` mirrors the profile fields accepted in `config.toml`:
  - Model selection, model provider, approval policy, reasoning effort/summary/verbosity overrides.
  - ChatGPT base URL and experimental instruction file paths.
  - Feature toggles controlling tools (`include_apply_patch_tool`, `tools_web_search`, etc.) and experimental features (unified exec, RMCP client).
  - Optional `FeaturesToml` table for the new `[features]` syntax.
- The struct derives `Deserialize`, enabling `ConfigToml` to hold a `HashMap<String, ConfigProfile>`. Profiles inherit defaults (via `Option` fields) and are merged atop base config in `Config::load_from_base_config_with_overrides`.
- `impl From<ConfigProfile> for codex_app_server_protocol::Profile` creates a trimmed version that backend services can consume (only core model settings are exported).

## Broader Context
- Profiles are selected via the `profile` key or CLI overrides. The chosen profile influences feature evaluation (`Features::from_config`) and config persistence (`config_edit` writes values under `[profiles.<name>]`).
- Profiles support hierarchical configuration: base defaults → profile overrides → session overrides.

## Technical Debt
- None identified; profile fields are intentionally minimal and map 1:1 with the TOML schema.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./config.rs.spec.md
  - ./features.rs.spec.md
