# Tool Implementation Patterns (Plan Tool Case Study)

This note captures the patterns we observed while digging into the built-in plan tool (`update_plan`). It now also outlines how we will apply those lessons to the multi-agent delegation tool so the primary assistant invokes sub-agents via structured tool calls.

## 1. Spec + Handler Separation
- Specification and handler live under `codex_core::tools`.
- `LazyLock<ToolSpec>` builds the JSON schema for arguments, mirroring the MCP declaration.
- Registration happens via `ToolRegistryBuilder` only when the active config sets `plan_tool = true`.

## 2. Schema-First Validation
- The spec enumerates the allowed statuses (`pending`, `in_progress`, `completed`) and requires each plan item to declare both `step` and `status`.
- Parsing uses `serde_json::from_str` into `UpdatePlanArgs`, so malformed payloads fail before any state is mutated.

## 3. Stateless Server Handler
- The handler simply converts the payload into a `PlanUpdate` event on the session bus and returns `"Plan updated"`.
- All “real” state (plan rendering, history, undo) stays in the client layers.

## 4. Config-Driven Inclusion
- `Config.include_plan_tool` toggles availability. Front ends (CLI, TUI, app server) set this flag through `ConfigOverrides`.
- For delegation we piggyback on `[multi_agent].agents`: the flag is auto-enabled whenever that list is non-empty, so child agents gain the delegate tool without extra overrides.
- When the feature is disabled, the tool spec and handler never register, preventing accidental invocation.

## 5. Client-Side Presentation
- The TUI listens for `EventMsg::PlanUpdate` to render a checklist-style history cell.
- Tests assert the event-to-UI path (`codex-rs/core/tests/suite/tool_harness.rs`, `codex-rs/tui/src/chatwidget/tests.rs`) so regressions surface quickly.

## 6. Takeaways for Delegation Tools
- Reuse the same pattern: declare a schema-rich `ToolSpec`, keep the handler stateless, and emit structured events for the UI.
- Guard inclusion with config or profile flags so we can stage features safely.
- Keep UX logic (streaming, history cells) in the client; server code just transports structured data.
- Treat delegation as an AI-triggered capability: the user cannot directly execute sub-agents; instead, the main model decides when to call the delegation tool based on conversational context.

## 7. Multi-Agent Delegate Tool Blueprint

### 7.1 Invocation Model
- The primary assistant issues a tool call (working name: `delegate_agent`) whenever it wants help from a sub-agent. Users supply plain language requests; the model chooses whether delegation is appropriate.
- The frontend passes user text verbatim. Guidance about which agent to choose lives in instructions rather than inline tokens.

### 7.2 Tool Spec Shape
```json
{
  "type": "object",
  "properties": {
    "agent_id": { "type": "string", "pattern": "^[a-z0-9_\\-]+$" },
    "prompt": { "type": "string" },
    "context": {
      "type": "object",
      "properties": {
        "working_directory": { "type": "string" },
        "hints": { "type": "array", "items": { "type": "string" } }
      },
      "additionalProperties": true
    },
    "batch": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["agent_id", "prompt"],
        "properties": {
          "agent_id": { "type": "string", "pattern": "^[a-z0-9_\\-]+$" },
          "prompt": { "type": "string" },
          "context": {
            "type": "object",
            "properties": {
              "working_directory": { "type": "string" },
              "hints": { "type": "array", "items": { "type": "string" } }
            },
            "additionalProperties": true
          }
        },
        "additionalProperties": false
      }
    }
  },
  "anyOf": [
    { "required": ["agent_id", "prompt"] },
    { "required": ["batch"], "properties": { "batch": { "minItems": 1 } } }
  ],
  "additionalProperties": false
}
```
- We can add optional fields later (timeouts, resource budgets) without breaking the schema.
- The handler validates `agent_id` with `AgentRegistry`, loads the merged `Config`, and passes the prompt/context into the orchestrator.
- A per-agent concurrency cap guards resource usage. `[multi_agent].max_concurrent_delegates` defaults to 5 and returns `DelegateToolError::DelegateInProgress` once the limit is hit, signalling the model to queue additional work.

### 7.3 Handler Responsibilities
- Mirror the exec tool: enqueue the delegate run, stream progress via `DelegateEvent::Started/Delta/Completed/Failed`, and return a compact JSON result. When batching requests, respond with `{"status":"ok","runs":[...]}` where each entry includes the `agent_id`, `run_id`, optional summary, and duration.
- Errors reuse the same shape with `status: "error"` so the UI can surface them consistently.
- The handler itself remains thin—after scheduling the work, it hands control back to the runtime.

### 7.4 Client Integration
- The TUI maintains a delegate tree so nested runs display with increasing indentation (two spaces per depth) and status indicators rotate between active roots. Streaming can hop between delegates; each run keeps its own buffer before being summarized into history.
- Because users cannot trigger the tool directly, slash commands and message preprocessing stay untouched; guidance lives in instructions and autocomplete metadata.

### 7.5 Instruction Updates
- Primary instructions clarify how to phrase requests when the assistant should consider delegation; there are no special inline tokens required.
- Sub-agent instructions remain focused on their specialised roles; the orchestrator constructs the prompt passed through the tool payload.
