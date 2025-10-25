## Overview
`codex_tool_config` defines the request schemas and config translation logic for the MCP `codex` and `codex-reply` tools. It exposes JSON-schema descriptions for clients and turns incoming parameters into `codex-core` runtime configuration.

## Detailed Behavior
- `CodexToolCallParam` (`kebab-case` fields) captures the initial prompt plus optional overrides for model, profile, working directory, approval policy, sandbox mode, inline config, and base instructions. It derives `JsonSchema` so the schema generator can produce the MCP tool definition.
- Enum wrappers:
  - `CodexToolCallApprovalPolicy` mirrors `AskForApproval` and implements `Into<AskForApproval>`.
  - `CodexToolCallSandboxMode` mirrors `SandboxMode` with a matching `Into` implementation.
- `create_tool_for_codex_tool_call_param`:
  - Uses `schemars` to generate a JSON schema for `CodexToolCallParam`, serializes it into `ToolInputSchema`, and returns an MCP `Tool` describing the `codex` entrypoint.
  - Leaves `output_schema` as `None` (not yet documented) with a TODO to fill in later.
- `CodexToolCallParam::into_config`:
  - Converts request parameters into `ConfigOverrides`, merges CLI-style overrides (via `json_to_toml`), and loads a full `codex_core::config::Config`.
  - Returns the user prompt separately so callers can immediately send it to the conversation.
- `CodexToolCallReplyParam` provides the schema for follow-up prompts on existing conversations. `create_tool_for_codex_tool_call_reply_param` mirrors the schema generation for the reply tool.
- Tests assert the exact tool JSON emitted for both schemas, ensuring that contractual changes are deliberate and reviewable.

## Broader Context
- `CodexMessageProcessor` in the MCP server uses `into_config` when launching or resuming Codex sessions (`./codex_tool_runner.rs.spec.md`).
- The generated schema is what MCP clients inspect to build forms and validations; keeping it in sync with `codex-common` presets and `codex-core` overrides prevents drift.
- Relies on configuration primitives from `codex-core` (`../../core/src/config.rs.spec.md`) and shared approval enums (`../../core/src/config_types.rs.spec.md`).

## Technical Debt
- Output schemas for the tools are still `None`, so clients must infer response structure from examples instead of JSON schema.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Define an explicit `output_schema` for the `codex` and `codex-reply` tools.
related_specs:
  - ./codex_tool_runner.rs.spec.md
  - ../../core/src/config.rs.spec.md
  - ../../core/src/config_types.rs.spec.md
