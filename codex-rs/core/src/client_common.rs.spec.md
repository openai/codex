## Overview
`core::client_common` houses shared request/response primitives for Codex’s model clients. It shapes prompts, defines the streaming event types surfaced to the runtime, and builds OpenAI-compatible payload fragments (tools, reasoning, text controls) used by both Responses and Chat Completions pipelines.

## Detailed Behavior
- `Prompt` aggregates conversation input (`ResponseItem` history), available tool specs, parallel-call policy, optional instruction overrides, and output schemas. Key helpers:
  - `get_full_instructions` stitches base instructions with `APPLY_PATCH_TOOL_INSTRUCTIONS` when the active model expects them and the freeform `apply_patch` tool is absent.
  - `get_formatted_input` clones the input stream and reserialises shell outputs when the freeform `apply_patch` tool is in play so the model sees structured text instead of raw JSON.
- Shell output normalization:
  - `reserialize_shell_outputs` tracks tool call IDs across local shell, `apply_patch`, and function outputs, rewriting the corresponding outputs via `parse_structured_shell_output`.
  - `build_structured_output` adds exit code, duration, optional line counts, and cleans up the “Total output lines” header for readability.
- Streaming surface:
  - `ResponseEvent` enumerates high-level events (`Created`, incremental deltas, `OutputItemDone`, reasoning markers, rate-limit snapshots).
  - `ResponseStream` wraps a `tokio::mpsc::Receiver<Result<ResponseEvent>>` and implements `Stream`, allowing consumers to poll events uniformly across wire protocols.
- Request payload helpers:
  - `Reasoning`, `TextControls`, `TextFormat`, and `OpenAiVerbosity` map configuration settings (`ReasoningEffortConfig`, `VerbosityConfig`, output schema) to the JSON expected by the Responses API.
  - `ResponsesApiRequest` captures the entire JSON body (borrowed references for zero-copy serialization).
  - `tools` submodule defines `ToolSpec` variants (function, local shell, web search, freeform) with helpers like `ToolSpec::name` to simplify downstream wiring.
  - `create_reasoning_param_for_request` / `create_text_param_for_request` gate optional fields based on model capabilities and caller-supplied overrides.
- Tests exercise instruction weaving, text controls serialization, and abstract the apply-patch logic across several model families.

## Broader Context
- `client.rs` and `chat_completions.rs` rely on these helpers to stay agnostic about prompt formatting and streaming semantics. Other modules (e.g., tool registry) construct `ToolSpec` lists before delegating to the client layer.
- The shared `ResponseEvent` and `ResponseStream` contracts let state management, telemetry, and UI code consume model output without caring which wire protocol produced it.
- Context can't yet be determined for future wire protocols (e.g., vendor-specific deltas); when introduced they should extend `ResponseEvent` rather than duplicating streaming logic elsewhere.

## Technical Debt
- Shell-output restructuring duplicates logic across multiple pattern matches; encapsulating the ID tracking in a dedicated helper or iterator adaptor would reduce the chance of missing new tool variants.
- `ResponsesApiRequest` still serializes `ResponseItem::Other`; replacing the enum with a serialization-specific view would make the boundary safer and avoid defensive comments.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Consolidate the shell output reserialization path into a reusable abstraction so additional tool variants (e.g., new MCP actions) inherit formatting automatically.
    - Introduce a serialization-layer DTO for prompt input to guarantee unsupported `ResponseItem` variants cannot leak into API requests.
related_specs:
  - ./client.rs.spec.md
  - ./chat_completions.rs.spec.md
  - ./model_provider_info.rs.spec.md
