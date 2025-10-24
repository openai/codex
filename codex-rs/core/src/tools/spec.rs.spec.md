## Overview
`core::tools::spec` builds the tool catalogue exposed to the model. It interprets model family capabilities and feature flags, assembles `ToolSpec`s, and registers handlers in the `ToolRegistryBuilder`. The module also defines lightweight JSON schema types used to describe tool parameters.

## Detailed Behavior
- `ToolsConfig` captures feature toggles (shell type, apply_patch variant, web search, view image, experimental tools) derived from `ToolsConfigParams` (model family + enabled features). `new` computes defaults based on model capabilities and feature flags.
- JSON schema helpers (`JsonSchema`, `AdditionalProperties`) describe tool parameter structures. Helpers like `create_exec_command_tool`, `create_write_stdin_tool`, and others return `ToolSpec` instances for core tools.
- `build_specs`:
  - Instantiates handlers (`ShellHandler`, `ApplyPatchHandler`, `PlanHandler`, MCP handlers, etc.), pushes corresponding specs into the builder, and registers handlers.
  - Adds optional tools (web search, view image, experimental read/list/grep/test_sync) based on `ToolsConfig`.
  - Integrates MCP tools by converting `mcp_types::Tool` definitions into OpenAI-compatible `ToolSpec::Function`s, registering a shared MCP handler for each.
- `ToolsConfig::shell_type` selects between default, local, or streamable shell tools, and `apply_patch_tool_type` chooses freeform vs. JSON apply_patch variants.
- Test helpers strip descriptions from schemas to compare structural equality and verify expected tool sets for various model families and feature combinations.

## Broader Context
- The router consumes tool specs to inform the model of available tools (`ToolRouter::specs`). Handlers registered here execute tool calls when the model invokes them.
- When new tools are added, they must be defined here with schema, handler registration, and tests to ensure inclusion for appropriate feature sets.
- Context can't yet be determined for dynamic tool registration; builder TODOs in `registry.rs` indicate potential future work, which would require making this builder extensible at runtime.

## Technical Debt
- A TODO notes that the JSON-based apply_patch tool should be deprecated once the freeform tool becomes universal; revisiting configuration defaults will simplify tool selection.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Remove the legacy JSON apply_patch tool path once all model families support the freeform implementation.
related_specs:
  - ./registry.rs.spec.md
  - ./handlers/mod.rs.spec.md
  - ../model_family/mod.spec.md
