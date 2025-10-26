## Overview
`codex_app_server_protocol::lib` gathers the protocol surface and export helpers so callers can import JSON-RPC types and code-generation utilities from a single module.

## Detailed Behavior
- Declares submodules `export`, `jsonrpc_lite`, and `protocol`.
- Re-exports:
  - `generate_types`, `generate_ts`, `generate_json` for producing TypeScript bindings and JSON Schemas.
  - All JSON-RPC helper types from `jsonrpc_lite`.
  - The full request/response/notification API from `protocol`.

## Broader Context
- Used by the app server, CLI clients, and build scripts (`src/bin/export.rs`) to access the shared protocol definitions and generators.

## Technical Debt
- None; module intentionally stays thin.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./protocol.rs.spec.md
  - ./export.rs.spec.md
  - ./jsonrpc_lite.rs.spec.md
