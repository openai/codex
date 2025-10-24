## Overview
`protocol::protocol` defines the core Codex session protocol: the submission queue (`Submission`, `Op`) that carries user requests into the agent, the event queue (`Event`, `EventMsg`) returned over SSE, and the shared policy types governing execution. It also bundles token usage accounting and command approval metadata so all clients interpret agent behavior consistently.

## Detailed Behavior
- Declares constants for user-instruction and environment-context tags used when constructing rich text prompts, keeping these markers consistent across crates.
- `Submission` wraps an `id` and an `Op` payload. `Op` enumerates all inbound operations, including conversation turns (`UserTurn`, `UserInput`), context overrides, approval responses, history lookups, MCP listings, plan updates, review mode transitions, and shutdown commands. Many variants inline key data such as `cwd`, `AskForApproval`, `SandboxPolicy`, and reasoning controls.
- `AskForApproval` and `SandboxPolicy` encode command-approval posture and execution restrictions. Helper methods on `SandboxPolicy` compute writable roots, default policies, and characteristic flags (disk/network access), while `WritableRoot` tracks read-only subpaths under otherwise writable directories.
- `Event` couples a submission ID with an `EventMsg`. `EventMsg` is a large tagged enum covering agent output (messages, reasoning, web search), execution progress (command begin/output/end, patch apply), approval requests, MCP results, plan updates (`PlanUpdate(UpdatePlanArgs)`), review mode transitions, token counts, history responses, and shutdown notifications. Each variant references typed payload structs defined later in the module or reuses types from other modules (e.g., `UpdatePlanArgs` from `plan_tool`).
- Token usage is tracked by `TokenUsage`, which records cached/non-cached inputs, reasoning tokens, and totals. Methods compute blended totals, context window usage, and percentage remaining. `FinalOutput` wraps `TokenUsage` for summary events and implements `Display` for log-friendly reporting.
- Numerous event payload structs (`AgentMessageEvent`, `AgentReasoningEvent`, `ExecCommandBeginEvent`, `StreamErrorEvent`, etc.) derive serialization traits to ensure the SSE stream stays self-describing and schema-safe.
- The module integrates with other protocol components: it imports `ContentItem`, `ResponseItem`, `TurnItem`, and `UserInput` to represent conversation content, as well as `UpdatePlanArgs` and `ParsedCommand` for plan and command reporting. MCP integrations surface via `CallToolResult` and associated event variants.

## Broader Context
- Serves as the canonical contract between `codex-core`, client shells, and the app server. Any change must be coordinated with TypeScript bindings and SSE consumers to avoid breaking live sessions.
- The queue model enables asynchronous bidirectional communication. Downstream specs (CLI, TUI, app-server) should reference the relevant `Op` and `EventMsg` variants they consume or emit to document compatibility expectations.
- `SandboxPolicy` and approval enums tie directly into execution and security layers. Specs for sandbox enforcement and approval tooling should cite these types to maintain behavioral parity.
- Context can't yet be determined for protocol versioning; introducing negotiated versions would require augmenting `Op` and `EventMsg` with handshake messages in a backward-compatible manner.

## Technical Debt
- None observed in this module; the protocol shapes are comprehensive and align with current runtime behavior.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./plan_tool.rs.spec.md
  - ./items.rs.spec.md
  - ./models.rs.spec.md
  - ./user_input.rs.spec.md
  - ./parse_command.rs.spec.md
  - ./config_types.rs.spec.md
  - ./num_format.rs.spec.md
