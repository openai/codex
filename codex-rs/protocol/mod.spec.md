## Overview
`codex-protocol` defines the shared data model that connects Codex clients, the core orchestration runtime, and external services. The crate concentrates protocol enums, request/response shapes, and utility types so downstream crates exchange strongly typed data without duplicating schemas. It intentionally avoids business logic, focusing instead on serialization and compatibility with JSON Schema and TypeScript bindings.

## Detailed Behavior
- `src/lib.rs` re-exports discrete modules for account metadata, configuration enums, conversation identifiers, command parsing, plan tool payloads, and the session protocol. Each module stays focused on a single conceptual area to limit coupling.
- Most types derive `Serialize`, `Deserialize`, `JsonSchema`, and `ts_rs::TS`, enabling a single source of truth for Rust, JSON, and TypeScript consumers. This keeps API surface changes synchronized across the CLI, TUI, app-server, and VS Code integrations.
- The `protocol` module captures the core submission and event queues, policy enums, token usage accounting, and command approval structures that mirror the wire protocol used over SSE and HTTP.
- Supplemental modules (`items`, `user_input`, `models`, `plan_tool`) provide higher-level representations of conversation turns, model IO, and task planning, which are consumed by both local UI components and remote services.
- Utility modules (`num_format`, `conversation_id`) encapsulate formatting and identifier generation so that formatting rules and UUID handling remain consistent across binaries.

## Broader Context
- This crate sits at the boundary between `codex-core` orchestration and user-facing shells. Changes in protocol types typically ripple into the app-server contract and must be mirrored in client toolchains such as the VS Code MCP extension.
- Types defined here are often serialized across process boundaries. Maintaining backward compatibility and versioning discipline is crucial; breaking changes require coordinated rollouts across all dependents.
- Context can't yet be determined for how future protocol versioning or negotiation will be handled; revisit once cross-version support designs are finalized.

## Technical Debt
- None observed at the crate level; each module confines itself to data definitions with limited logic.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/protocol.rs.spec.md
  - ./src/config_types.rs.spec.md
  - ./src/models.rs.spec.md
  - ./src/items.rs.spec.md
  - ./src/plan_tool.rs.spec.md
  - ./src/user_input.rs.spec.md
  - ./src/custom_prompts.rs.spec.md
  - ./src/message_history.rs.spec.md
  - ./src/parse_command.rs.spec.md
  - ./src/account.rs.spec.md
  - ./src/conversation_id.rs.spec.md
  - ./src/num_format.rs.spec.md
