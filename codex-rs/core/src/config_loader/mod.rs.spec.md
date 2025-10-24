## Overview
`core::config_loader` loads configuration layers from disk and managed sources. It handles user config (`config.toml`), system-managed overrides, and macOS managed preferences, returning a merged `TomlValue` ready for higher-level deserialisation.

## Detailed Behavior
- `LoadedConfigLayers` groups the three tiers: base config, optional managed config (`managed_config.toml`), and optional managed preferences (macOS device profiles).
- `LoaderOverrides` allows tests/CLI to point at alternate managed config paths or inject managed preferences via base64.
- `load_config_layers_with_overrides` reads the user config and managed config asynchronously, optionally loading managed preferences (via `macos::load_managed_admin_config_layer`). Missing files are logged and treated as empty tables.
- `read_config_from_path` handles IO/parsing errors with structured tracing messages, distinguishing between missing files (info/debug) and parsing failures (error).
- `merge_toml_values` recursively merges overlay tables into the base (overlay wins), used by `apply_managed_layers` and other utilities.
- `managed_config_default_path` points to `/etc/codex/managed_config.toml` on Unix and `managed_config.toml` under `CODEX_HOME` on other platforms.
- Top-level helpers:
  - `load_config_as_toml`/`load_config_as_toml_with_overrides` return a single `TomlValue` with overlays applied (managed config & managed preferences stacked over base).
  - `load_config_as_toml_with_overrides` is used by `Config::load_with_cli_overrides` to seed the overlay pipeline before CLI/application overrides are applied.

## Broader Context
- This module isolates IO concerns so `config.rs` can focus on schema validation and merging CLI/profile overrides. Managed layers allow enterprise deployments to enforce settings without modifying user files.
- On macOS, managed preferences (MDM payloads) are base64-encoded TOML entries fetched via CoreFoundation APIs (see `macos.rs`).

## Technical Debt
- None noted; the layering strategy is straightforward and easily extended if new config tiers are introduced.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./macos.rs.spec.md
  - ../config.rs.spec.md
