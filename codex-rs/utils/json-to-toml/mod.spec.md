## Overview
`codex-utils-json-to-toml` exposes helpers for turning arbitrary JSON documents into TOML data structures so configuration overrides can move between HTTP payloads and the on-disk TOML format Codex expects.

## Detailed Behavior
- Re-exports `json_to_toml` from `src/lib.rs`, which recursively maps JSON values into their TOML counterparts while preserving booleans, numbers, arrays, and nested objects.
- Provides workspace crates a stable way to translate request bodies into TOML tables before they are merged into Codex configuration files or profiles.

## Broader Context
- Used by service entrypoints such as `app-server` and `mcp-server` when they accept JSON-formatted tool parameters or profile overrides and need to reconcile them with TOML-backed config files.

## Technical Debt
- None identified for this crate; callers handle validation of converted values.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
