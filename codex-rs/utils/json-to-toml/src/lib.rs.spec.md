## Overview
`json_to_toml` converts arbitrary `serde_json::Value` inputs into `toml::Value` instances so upstream services can merge JSON-formatted overrides into TOML configuration files without re-implementing conversion logic.

## Detailed Behavior
- Maps JSON nulls to empty TOML strings, mirroring the legacy behavior expected by Codex configuration persistence.
- Preserves booleans and strings by returning the equivalent TOML variants.
- Converts numbers preferentially to integers (via `as_i64`) and falls back to floats (`as_f64`), stringifying any non-representable numeric encodings.
- Recursively walks arrays and objects, invoking `json_to_toml` on each element or value to build TOML arrays and tables.
- Includes unit tests covering scalar, array, and nested object conversions to ensure serde/toml upgrades do not subtly change semantics.

## Broader Context
- Invoked by `app-server` request handling when RPC payloads attach JSON overrides that must be reconciled with TOML-backed workspace profiles.
- Used by `mcp-server` to translate MCP tool configuration payloads into TOML before composing `codex_core::config::Config` instances.

## Technical Debt
- None identified; the conversion logic intentionally mirrors historical behavior and remains covered by regression tests.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ../../app-server/src/codex_message_processor.rs.spec.md
  - ../../mcp-server/src/codex_tool_config.rs.spec.md
