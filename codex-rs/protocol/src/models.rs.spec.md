## Overview
`protocol::models` describes how Codex represents model inputs and outputs when exchanging data with OpenAI Responses, Chat Completions, and MCP tooling. It defines the shapes for response items, content blocks, tool call payloads, and shell execution metadata, ensuring both Rust and TypeScript clients interpret streamed events the same way.

## Detailed Behavior
- `ResponseInputItem` and `ResponseItem` enumerate the message, tool call, reasoning, and web-search structures emitted by model endpoints. Each variant mirrors the SSE payloads returned by OpenAI, including optional IDs for protocols that supply them.
- `ContentItem` captures text and image blocks. When constructing a response from `UserInput`, the `From<Vec<UserInput>>` implementation encodes local image files as base64 data URIs, inferring MIME types with `mime_guess` and warning (via `tracing`) when a file cannot be read.
- Tool-related types (`FunctionCall`, `FunctionCallOutput`, `CustomToolCall`, `LocalShellCall`, `McpToolCallOutput`, `CustomToolCallOutput`, `WebSearchCall`) preserve the raw strings or JSON payloads expected by the upstream APIs. `FunctionCallOutputPayload` implements custom `Serialize`/`Deserialize` to always emit and accept plain strings, matching the JavaScript CLI’s behavior and avoiding API 400s.
- `LocalShellExecAction` tracks command invocation details, including optional escalated-permissions metadata that downstream execution engines honor.
- Reasoning content types (`ReasoningItemReasoningSummary`, `ReasoningItemContent`) help clients display summary vs. raw reasoning, while helper `should_serialize_reasoning_content` controls when the optional content list is serialized.
- Implements `From<ResponseInputItem>` for `ResponseItem` to normalize tool call outputs and convert MCP tool results into the Responses shape automatically.

## Broader Context
- These structs serve as the lingua franca between Codex’s core orchestration and UI layers, and they also drive the TypeScript bindings consumed by Electron and VS Code clients. Any change requires verifying parity with upstream API expectations and generated schemas.
- The custom serialization and base64 conversion code is security-sensitive: failures can leak paths or break parity with OpenAI payloads. Specs for execution engines and file handling should reference this module to stay aligned.
- Context can't yet be determined for future tool call types (e.g., streaming web searches or multi-modal outputs); new enum variants must be added in lockstep with service support.

## Technical Debt
- `FunctionCallOutputPayload` still carries an optional `success` flag that is never serialized (see inline TODO). Removing the field once downstream code stops depending on it would simplify the payload and avoid confusion.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Remove the unused `success` flag from `FunctionCallOutputPayload` when dependents no longer rely on it.
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./user_input.rs.spec.md
  - ./protocol.rs.spec.md
