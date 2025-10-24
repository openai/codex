## Overview
`common::config_override` implements the `CliConfigOverrides` struct and helpers that gather `-c key=value` arguments from Clap-based binaries. It normalizes the raw strings into parsed values and applies them onto mutable TOML configuration trees so callers can layer overrides after reading `config.toml`.

## Detailed Behavior
- `CliConfigOverrides` derives `Parser` and `ValueEnum` support from Clap, registering `-c/--config key=value` as an appendable, global flag. Every occurrence becomes a raw string stored in `raw_overrides`.
- `parse_overrides` iterates over the raw strings, splitting on the first `=` to separate the dotted path from the value. Empty keys or missing separators yield descriptive errors so CLI tools can surface validation issues to users.
- Attempts to parse the value using `parse_toml_value`, which wraps the fragment inside a sentinel assignment (`_x_ = ...`) and feeds it to `toml::from_str`. If parsing fails—typically because the value is an unquoted bare string—the fallback trims surrounding quotes and stores it as `Value::String`, accepting common shorthand like `-c model=o3`.
- `apply_on_value` drives the override application: it expands the parsed list and calls `apply_single_override` for each `(path, value)`, creating intermediate tables as needed. Existing values at the destination path are replaced to reflect the override semantics.
- `apply_single_override` walks the dotted path, materializing missing intermediate tables in place. When the target key points to a non-table value, the function replaces it with a fresh table so the override can proceed.
- Unit tests behind the `cli` feature verify the parsing logic across scalars, arrays, and inline tables to guard against regressions in the TOML behavior.

## Broader Context
- This module underpins CLI tools that let users tweak configuration without editing files. It must stay in sync with `codex-core`'s configuration schema so overrides apply to the same nested keys the loader expects.
- Reuses `toml::Value` directly because the persistent configuration is stored in TOML. Downstream modules that convert into typed structs should document any additional validation they perform after overrides are applied.
- Context can't yet be determined for how conflicting overrides across multiple layers (CLI vs. environment vs. profile) should be merged; defer to specs for `codex-core` configuration loading once available.

## Technical Debt
- The module-level documentation still references `serde_json::Value`, but the implementation now works with `toml::Value`. Updating the docs would avoid confusion for maintainers inspecting the public API.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Update module documentation to describe TOML-based parsing instead of JSON terminology.
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./format_env_display.rs.spec.md
