## Overview
`core::features` centralizes feature flags for Codex. It defines the canonical feature set, applies legacy toggles, and produces a `Features` container used by `Config` to decide which experimental tools or behaviours are active.

## Detailed Behavior
- `Feature` enumerates toggles (`UnifiedExec`, `StreamableShell`, `RmcpClient`, `ApplyPatchFreeform`, `ViewImageTool`, `WebSearchRequest`) and exposes metadata (`key`, `stage`, `default_enabled`) defined in the `FEATURES` registry.
- `Features` maintains a `BTreeSet<Feature>` of enabled flags. It exposes:
  - Constructors (`with_defaults`, `from_config`) that seed the set with defaults, then apply base config toggles, profile toggles, legacy aliases, and CLI overrides (`FeatureOverrides`).
  - Methods to enable/disable and query features, plus `apply_map` for TOML-style `[features]` tables.
- `FeatureOverrides` bridges command-line/session overrides to the new system, translating booleans into legacy toggles before applying them to the set.
- Legacy support:
  - `legacy::LegacyFeatureToggles` maps historic booleans (e.g., `experimental_use_unified_exec_tool`, `include_view_image_tool`) to the new feature set, logging whenever aliases are used so users migrate to `[features].<key>`.
  - `legacy::feature_for_key` resolves old keys when parsing `[features]` tables, preserving backward compatibility.
- Stages (`Stage::Experimental/Beta/Stable/Deprecated/Removed`) annotate feature lifecycles, aiding documentation and dashboards.

## Broader Context
- `Config::load_from_base_config_with_overrides` relies on `Features::from_config` to compute tool toggles. Tool registration then checks `Features` to decide which handlers to expose.
- By funnelling all toggles through a single container, feature gating stays consistent across CLI, TUI, MCP, and managed deployments.
- Legacy logging helps administrators spot outdated keys, easing migration to the new `[features]` syntax.

## Technical Debt
- None noted; the module already isolates legacy compatibility, making future feature additions straightforward.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./config.rs.spec.md
  - ./config_profile.rs.spec.md
