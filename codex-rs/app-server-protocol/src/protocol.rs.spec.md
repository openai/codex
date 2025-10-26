## Overview
`protocol` defines the JSON-RPC request, response, and notification types exchanged between Codex app clients and the app server. It captures authentication flows, conversation lifecycle, tool approvals, file operations, and configuration management while deriving Serde, JSON schema, and TypeScript metadata.

## Detailed Behavior
- `client_request_definitions!` macro enumerates every client → server method, generating:
  - `ClientRequest` variants with method names (`model/list`, `account/login`, `conversation/new`, etc.) plus typed `params` payloads.
  - Helpers to export matching response schemas (`export_client_responses`, `export_client_response_schemas`).
- `server_request_definitions!` (later in file) mirrors the pattern for server → client requests (e.g., `ExecCommandApproval`, `ApplyPatchApproval`, `SendUserMessage`), along with schema exporters.
- Notifications are declared via `server_notification_definitions!` and `client_notification_definitions!`, covering events like login state changes, conversation updates, streaming outputs, and session configuration.
- Data structures include:
  - Auth/account models (`AuthMode`, `LoginAccountParams`, `AuthStatusChangeNotification`, rate limit snapshots).
  - Conversation management (`NewConversationParams`, `ResumeConversationParams`, `ConversationSummary`, `SessionConfiguredNotification`).
  - Tool and command approvals (`ExecCommandApprovalParams`, `PatchApprovalElicitRequestParams`, `Tools` settings).
  - File search, diffing, git status, and sandbox policies, all reusing types from `codex_protocol`.
- Extensive derives (`Serialize`, `Deserialize`, `JsonSchema`, `TS`) ensure the export pipeline can generate artifacts without manual wiring.
- Utility `RequestWithId`/`ServerRequestPayload` glue types provide ergonomic conversions between typed params and JSON-RPC envelopes.
- Embedded unit tests assert canonical JSON serialization for key requests, preserving backwards compatibility of field names and method strings.

## Broader Context
- The app server and clients use these types directly when marshalling JSON-RPC payloads. Generated TypeScript definitions keep Electron/Tauri clients and web UIs synchronized with the Rust source of truth.

## Technical Debt
- The file centralizes a large amount of API surface. Future refactors could break it into thematic modules (auth, conversations, tools) to improve navigation, but exports would need adjusting carefully.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Consider splitting protocol definitions into submodules (auth, conversations, tools) to reduce compile times and aid discoverability without breaking generated exports.
related_specs:
  - ../mod.spec.md
  - ./export.rs.spec.md
  - ./jsonrpc_lite.rs.spec.md
