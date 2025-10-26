## Overview
`jsonrpc_lite` provides minimal JSON-RPC 2.0 data structures tailored to the app-server protocol. It omits the `jsonrpc` version field in serialized payloads, matching Codex’s wire format.

## Detailed Behavior
- Defines `RequestId` (string or integer) plus the canonical message enums (`JSONRPCMessage`, `JSONRPCRequest`, `JSONRPCNotification`, `JSONRPCResponse`, `JSONRPCError`).
- Exposes `JSONRPC_VERSION` and a `Result` alias (`serde_json::Value`) for convenience.
- All structs derive `Serialize`, `Deserialize`, `JsonSchema`, and `TS` to feed code generation in `export.rs`.

## Broader Context
- Reused by both server and client code when marshaling JSON-RPC messages, and exported via TypeScript/JSON schema generators.

## Technical Debt
- None; module intentionally focuses on thin data definitions.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./export.rs.spec.md
