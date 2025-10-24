## Overview
`core::tools::router` translates model-produced `ResponseItem`s into executable `ToolCall`s and dispatches them to registered handlers. It bridges protocol tool invocations with the configured registry, handles MCP tool naming, and produces fallback outputs on failure.

## Detailed Behavior
- `ToolRouter::from_config` builds tool specs and registry handlers via `build_specs`, optionally incorporating MCP tool metadata. The resulting router stores both the registry and the configured tool specs, exposing `specs()` for prompt generation and `tool_supports_parallel` checks.
- `build_tool_call` converts response items into `ToolCall` structs:
  - Function calls detect MCP names (`session.parse_mcp_tool_name`) and map to either `ToolPayload::Mcp`, `ToolPayload::UnifiedExec`, or `ToolPayload::Function`.
  - Custom tool calls transform into `ToolPayload::Custom`.
  - Local shell calls normalize the call ID (falling back to the response item ID) and wrap parameters in `ToolPayload::LocalShell`.
  - Non-tool response items return `Ok(None)` so the caller can treat them as regular messages.
- `dispatch_tool_call` constructs a `ToolInvocation`, hands it to `ToolRegistry::dispatch`, and converts results or errors into the appropriate `ResponseInputItem`. If handlers return non-fatal errors (e.g., rejections), a synthetic failure payload is sent back to the model (`failure_response`), using `CustomToolCallOutput` when the original payload was custom.
- Parallel tool support is recognized by checking configured specs with the `supports_parallel_tool_calls` flag; the router itself leaves scheduling to upstream orchestration.

## Broader Context
- The router is invoked within `codex.rs::run_turn`, forming the core of the tool loop. Specs for `registry.rs` and the various handlers detail how invocations execute once routed.
- MCP tooling relies on the router’s ability to detect server-qualified names; changes to naming conventions must be mirrored here to avoid misrouting.
- Context can't yet be determined for dynamically registered tools; TODOs in the registry suggest future enhancements that would require router updates to support runtime-added handlers.

## Technical Debt
- Error handling for MCP argument parsing (`ToolPayload::Mcp`) bubbles up as fatal errors. Enhancing validation before dispatch could produce clearer messages and avoid invoking the registry when arguments are obviously invalid.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Add upfront validation for MCP payload JSON to catch serialization errors before invoking handlers.
related_specs:
  - ./context.rs.spec.md
  - ./registry.rs.spec.md
  - ../codex.rs.spec.md
