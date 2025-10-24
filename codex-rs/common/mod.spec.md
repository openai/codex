## Overview
`codex-common` provides a small collection of shared utilities that multiple Codex binaries consume. The crate centralizes CLI helpers, configuration summarization, sandbox reporting, fuzzy matching, and model metadata so that other crates (`codex-cli`, `codex-tui`, `codex-mcp-server`) can depend on one crate instead of reimplementing light-weight logic. All exports are orchestrated through `src/lib.rs`, which re-exports modules behind feature flags to keep binary footprints lean.

## Detailed Behavior
- Feature flags gate optional dependencies: `cli` enables Clap-based argument types and JSON/TOML parsing helpers, `elapsed` exposes human-readable timing functions, and `sandbox_summary` returns a short description of sandbox policies. The default build includes only always-on utilities like configuration summarization and presets.
- CLI integrations (e.g., `CliConfigOverrides`, `ApprovalModeCliArg`, `SandboxModeCliArg`) expose ergonomic wrappers so downstream Clap definitions can flatten shared arguments while keeping parsing logic centralized in this crate.
- Configuration utilities (`config_summary`, `config_override`) translate structured configuration data into user-facing summaries and apply layered overrides. They depend on `codex-core` types to ensure parity with the runtime configuration schema.
- Preset catalogs (`model_presets`, `approval_presets`) define curated defaults for models and safety policies. These structs are pure data to keep UI code simple and promote consistency across applications.
- Shared helpers such as `fuzzy_match` and `format_env_display` provide UI-friendly formatting and search routines while encapsulating Unicode edge cases and presentation rules.

## Broader Context
- The crate sits between foundational types (`codex-core`, `codex-protocol`, `codex-app-server-protocol`) and user-facing binaries. Changes here often require coordinating feature flags and data contracts with those upstream crates.
- Consumers rely on consistent feature combinations: binaries that need CLI support must activate the `cli` feature, while minimal environments omit it to avoid pulling in Clap. Documenting these expectations in each consumer spec will help prevent accidental omissions.
- Context can't yet be determined for how sandbox summaries integrate with platform-specific sandbox tooling until platform modules (e.g., Seatbelt policies) receive their own specs.

## Technical Debt
- No crate-wide technical debt is evident beyond module-level items already tracked within their respective specs.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/config_summary.rs.spec.md
  - ./src/sandbox_summary.rs.spec.md
  - ./src/format_env_display.rs.spec.md
  - ./src/config_override.rs.spec.md
  - ./src/approval_presets.rs.spec.md
  - ./src/model_presets.rs.spec.md
  - ./src/fuzzy_match.rs.spec.md
  - ./src/elapsed.rs.spec.md
  - ./src/approval_mode_cli_arg.rs.spec.md
  - ./src/sandbox_mode_cli_arg.rs.spec.md
