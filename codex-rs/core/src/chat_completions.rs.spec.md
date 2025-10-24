## Overview
`core::chat_completions` adapts Codex prompts to the OpenAI-compatible Chat Completions API. It serializes conversation history into the `messages` array, handles streaming deltas, and exposes utilities to aggregate raw deltas into the `ResponseEvent` stream shared across the runtime.

## Detailed Behavior
- `stream_chat_completions` is the primary entrypoint:
  - Rejects `Prompt::output_schema` because the Chat API lacks structured-response support.
  - Builds the `messages` array: injects system instructions, iterates prior `ResponseItem`s, maps reasoning blocks onto adjacent assistant/tool anchors, and deduplicates repeated assistant text.
  - Converts tool metadata via `create_tools_json_for_chat_completions_api`.
  - Issues the POST request using provider configuration (`ModelProviderInfo::create_request_builder`), logs latency through `OtelEventManager`, and retries with exponential backoff while honoring `Retry-After`.
  - On success, spawns `process_chat_sse` to translate streaming deltas into `ResponseEvent`s.
- Reasoning attachment:
  - Collects post-user reasoning segments and associates them with the correct assistant or tool-call message to preserve context in the serialized history.
  - Maintains `reasoning_by_anchor_index` to stitch freeform reasoning text into JSON payloads.
- `process_chat_sse` reads server-sent events with an idle timeout:
  - Accumulates assistant text and reasoning deltas, emitting `ResponseEvent::OutputTextDelta` and `ResponseEvent::ReasoningContentDelta` as chunks arrive.
  - Tracks function/tool-call deltas (`function_call`, `tool_calls`) across multiple events so a complete `ResponseItem::FunctionCall` is emitted once finish reasons indicate completion.
  - Emits synthesized `ResponseEvent::Completed` events (Chat streaming does not include IDs or usage data).
  - Handles `[DONE]` sentinel to flush aggregated assistant/reasoning output before closing.
- Aggregation adapters:
  - `AggregateStreamExt` extends `ResponseStream` with `.aggregate()` and `.streaming_mode` to reconcile Chat streams with Responses semantics.
  - `AggregatedChatStream` buffers deltas, converts them into final `ResponseItem::Message` or `ResponseItem::Reasoning`, and controls whether downstream consumers see deltas, the aggregated result, or both.
- Tests cover reasoning attachment, SSE parsing, aggregation behaviors, retry delays, and payload serialization to guard the request contract.

## Broader Context
- `ModelClient` falls back to this module when `ModelProviderInfo::wire_api` is `Chat`; the resulting `ResponseStream` is indistinguishable from Responses-mode streams, keeping downstream orchestration uniform.
- Tool schemas leverage the same definitions as the Responses pipeline, ensuring the model sees consistent tool metadata regardless of wire protocol.
- Context can't yet be determined for provider-specific extensions (e.g., vendor reasoning attributes); compatibility layers should extend the aggregation logic rather than bypassing `AggregatedChatStream`.

## Technical Debt
- The message-building pass in `stream_chat_completions` is dense, blending history transformation, reasoning stitching, and deduplication; extracting dedicated helpers (e.g., for reasoning anchoring) would reduce cognitive load.
- `process_chat_sse` encodes finish-reason heuristics inline; wiring a declarative state machine would make multi-provider support easier to evolve.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Break the message serialization pipeline into named helpers for reasoning attachment, tool-call mapping, and deduplication to simplify future changes.
    - Replace the ad-hoc SSE handling with a state machine that captures finish reasons and delta accumulation explicitly, reducing the risk of drift when OpenAI alters payloads.
related_specs:
  - ./client.rs.spec.md
  - ./client_common.rs.spec.md
  - ./model_provider_info.rs.spec.md
