## Overview
`core::config_types` defines the TOML-facing structs and enums that underpin `Config`. It serializes/deserializes nested configuration blocks (MCP servers, sandbox workspace write settings, TUI options, OTEL exporters, notifications, etc.) without embedding business logic.

## Detailed Behavior
- MCP configuration:
  - `McpServerConfig` bundles transport (`Stdio` or `StreamableHttp`), enabled flags, startup/tool timeouts, and tool allow/deny lists.
  - Custom `Deserialize` logic normalises legacy fields, forbids unsupported combinations (e.g., stdio-specific fields on HTTP transports), converts timeouts from seconds/milliseconds, and reuses helpers (`option_duration_secs`).
- Auxiliary enums/structs:
  - `McpServerTransportConfig` is an untagged enum covering stdio transports (command/args/env/cwd) and streamable HTTP transports (URL, bearer token env var, headers).
  - `ShellEnvironmentPolicy` and related types define inheritance patterns, include/exclude globs, and overrides used by `exec_env::create_env`.
  - `History`, `Notifications`, `OtelConfig`, `OtelExporterKind`, `UriBasedFileOpener`, `SandboxWorkspaceWrite`, `ProjectConfig`, etc., capture structured options consumed by other modules.
  - `ReasoningSummaryFormat`, `OtelConfigToml`, and other helper structs convert between in-memory representations and TOML.
- Utility modules (`option_duration_secs`, wildmatch wrappers) provide serde adapters for `Duration` and glob matching.
- Tests ensure parsing covers common cases (history persistence, sandbox workspace write options, etc.).

## Broader Context
- `ConfigToml` composes these types when deserialising raw config, while `Config` converts them into runtime structures. Modules dealing with sandboxing, telemetry, history, and tools rely directly on these definitions.
- Keeping this file free of business logic simplifies testing and avoids cyclic dependencies—anything requiring logic (e.g., sandbox policy derivation) lives in `config.rs`.

## Technical Debt
- None noted; the module is intentionally declarative and should remain so to keep configuration schema changes manageable.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./config.rs.spec.md
  - ./exec_env.rs.spec.md
  - ./sandboxing/mod.rs.spec.md
