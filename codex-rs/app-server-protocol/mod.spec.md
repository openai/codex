## Overview
`codex-app-server-protocol` defines the JSON-RPC contract between Codex app clients and the app server, and provides generators for producing TypeScript bindings and JSON Schemas consumed by frontends and tooling.

## Detailed Behavior
- `src/lib.rs` re-exports the protocol definitions, JSON-RPC helpers, and code-generation utilities so downstream crates can depend on a single entrypoint.
- `src/protocol.rs` declares every client/server request, response, and notification type using serde/schemars/ts-rs annotations, along with helper macros for exporting response schemas.
- `src/jsonrpc_lite.rs` implements a lightweight JSON-RPC 2.0 wrapper (`JSONRPCMessage`, `JSONRPCRequest`, etc.) tailored to Codex.
- `src/export.rs` generates TypeScript type definitions and JSON Schemas for all protocol structures, wiring in optional formatting via Prettier.
- `src/bin/export.rs` exposes a CLI for running the generators.

## Broader Context
- The app server reads/writes these types when communicating with desktop and web clients; generated artifacts are checked in under `codex-rs/docs` and the frontend repos.

## Technical Debt
- Keeping Rust definitions, generated TypeScript, and JSON schemas in sync requires running the export CLI whenever protocol changes land.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Automate TypeScript/JSON schema regeneration (via CI or git hooks) so protocol changes cannot land without refreshed artifacts.
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/protocol.rs.spec.md
  - ./src/export.rs.spec.md
  - ./src/jsonrpc_lite.rs.spec.md
  - ./src/bin/export.rs.spec.md
