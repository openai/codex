## Overview
`mcp_types::lib` is the generated source that mirrors the official Model Context Protocol schema. It defines every request, notification, capability object, and content block used by MCP so Codex components can speak JSON-RPC 2.0 with MCP servers and clients. The file is regenerated from `schema/<version>/schema.json` via `./generate_mcp_types.py`, keeping the Rust data model aligned with the upstream specification.

## Detailed Behavior
- Declares the `ModelContextProtocolRequest` and `ModelContextProtocolNotification` traits. Each generated request/notification type implements the appropriate trait with an associated `METHOD` constant and parameter/result structs, enforcing method names at compile time.
- Enumerates top-level protocol constructs such as `ClientRequest`, `ServerNotification`, `ClientNotification`, and `ServerRequest`. Each enum uses serde tagging (`method` + `params`) to match the JSON-RPC wire format. Helpers provide `TryFrom<JSONRPCRequest>` / `TryFrom<JSONRPCNotification>` implementations that dispatch based on the method string and deserialize the corresponding payload.
- Implements shared schema objects: content blocks (`TextContent`, `ImageContent`, `AudioContent`, etc.), annotations, resource listings, tool interfaces, completion payloads, sampling configuration, and structured error types. All structs derive serde, `JsonSchema`, and `TS` to generate consistent JSON Schema and TypeScript bindings.
- Captures JSON-RPC plumbing types (`JSONRPCRequest`, `JSONRPCResponse`, `RequestId`, `ErrorData`, etc.), including defaulting helpers like `default_jsonrpc()` to guarantee the `"jsonrpc": "2.0"` field is present across messages.
- Uses `#[expect(clippy::unwrap_used)]` and similar annotations sparingly where the schema guarantees correctness (e.g., converting a generated struct to JSON never fails). These expectations keep the generated code warning-free without altering semantics.

## Broader Context
- The generated types are consumed by `codex-mcp-server`, VS Code MCP integrations, and future MCP-aware tooling. Regenerating the file when `SCHEMA_VERSION` changes ensures compatibility with the latest protocol release.
- JSON-RPC conversions and enums provide a single source of truth for method naming; higher-level crates should rely on these traits and enums instead of hardcoding method strings.
- Context can't yet be determined for multi-version support or experimental schema extensions. When the MCP spec introduces negotiated versions, the generator and these types will need augmentation to model coexistence.

## Technical Debt
- None observed; any maintenance focuses on updating the generator script when the upstream schema evolves.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
