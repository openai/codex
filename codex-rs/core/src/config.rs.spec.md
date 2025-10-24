## Overview
`core::config` is the central configuration hub for Codex. It loads user settings from `~/.codex/config.toml`, merges managed layers, applies CLI overrides, and produces a strongly typed `Config` used throughout the runtime. It also exposes helpers for persisting updates (e.g., model selection, profile overrides) and managing MCP server definitions.

## Detailed Behavior
- `Config` aggregates every knob needed at runtime: model selection, provider info, sandbox/approval policies, shell environment rules, feature flags, MCP server configuration, telemetry (OTEL), notifications, project trust metadata, and more. Most fields are derived from `ConfigToml`, optionally merged with `ConfigOverrides`, active profiles, and feature toggles.
- Loading pipeline:
  - `Config::load_with_cli_overrides` → `load_resolved_config` leverages `config_loader` to load base, managed config, and managed preferences, then calls `apply_overlays` to merge CLI overrides and managed layers (later layers win).
  - The resulting `TomlValue` is deserialised into `ConfigToml`; `load_from_base_config_with_overrides` combines `ConfigToml`, `ConfigOverrides`, and `ConfigProfile` to compute the final `Config`, including feature evaluation via `Features::from_config`, model family derivation, sandbox policy resolution, project trust detection, and optional instructions loading.
- Overlay helpers:
  - `apply_overlays` applies CLI overrides (using dotted-path updates) and merges managed config/preferences via `merge_toml_values`.
  - `apply_toml_override` and `apply_toml_edit_override_segments` update TOML values with explicit key segments so dotted keys are interpreted correctly.
- MCP management:
  - `load_global_mcp_servers` retrieves the `[mcp_servers]` table, verifying no inline `bearer_token` secrets linger.
  - `write_global_mcp_servers` rewrites the section while preserving formatting, serialising transport-specific fields (`stdio` vs `streamable_http`) and ensuring `managed_config` overlays remain intact.
- Persistence helpers:
  - `persist_model_selection` writes `model`/`model_reasoning_effort` to either the active profile or top level, creating directories as needed.
  - Lower-level utilities (`ensure_profile_table`, `set_project_trusted`, `remove_toml_edit_segments`) reshape the TOML document without discarding existing comments.
- `ConfigToml` mirrors the raw configuration schema, including `[profiles]`, feature maps, TUI settings, MCP transports, and project trust metadata. It provides helpers like `derive_sandbox_policy`, `load_project_doc_overrides`, `get_active_project`, and `get_config_profile`.
- `ConfigOverrides` captures CLI/session overrides (model, review model, cwd, sandbox mode, additional writable roots, etc.) which are folded into the final `Config`.
- Miscellaneous utilities: `find_codex_home` resolves the Codex home directory respecting `CODEX_HOME`; `log_dir` builds the log path; `ensure_no_inline_bearer_tokens` warns about deprecated MCP secrets.

## Broader Context
- Almost every subsystem consumes `Config`; changes in this file ripple across execution, tools, and UI. The layering strategy (base → managed config → managed preferences → CLI) ensures enterprise deployments can enforce defaults while allowing per-session overrides.
- Feature toggles live in `features.rs`; sandbox policies rely on `landlock.rs`/`seatbelt.rs`; project trust interacts with `project_doc`.
- CLI tools use persistence helpers (`persist_model_selection`, `persist_overrides` in `config_edit.rs`) to modify configuration safely without clobbering user formatting.

## Technical Debt
- Multiple TODOs note the need to refactor config persistence (e.g., `persist_model_selection`, `persist_overrides`). A dedicated writer abstraction would simplify maintaining comments and profiles.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Refactor configuration persistence helpers to avoid ad-hoc `toml_edit` manipulation scattered throughout the file (see TODOs such as `TODO(jif) refactor config persistence`).
related_specs:
  - ./config_loader/mod.rs.spec.md
  - ./config_loader/macos.rs.spec.md
  - ./config_edit.rs.spec.md
  - ./config_profile.rs.spec.md
  - ./config_types.rs.spec.md
  - ./features.rs.spec.md
