## Overview
`core::client` orchestrates outbound model requests for Codex. `ModelClient` encapsulates provider metadata, auth, telemetry, and retry logic, choosing between the OpenAI Responses API and legacy Chat Completions. It streams responses back as `ResponseStream` events, translating SSE payloads into the runtime’s internal protocol.

## Detailed Behavior
- `ModelClient::stream` delegates to `stream_with_task_kind`, which inspects `ModelProviderInfo::wire_api` and routes to either `stream_responses` (Responses API) or `chat_completions::stream_chat_completions`. Chat flows are wrapped in `AggregateStreamExt::aggregate` unless raw reasoning output is requested.
- `stream_responses` builds the payload:
  - Derives instructions and input via `Prompt::get_full_instructions` / `get_formatted_input`.
  - Creates tool schemas with `create_tools_json_for_responses_api` and reasoning/text controls via helpers from `client_common`.
  - Applies Azure-specific workarounds (forcing `store: true`, preserving IDs) and attaches conversation IDs so Codex’s prompt cache can be reused.
  - Retries with exponential backoff (`backoff`) or `Retry-After` hints; the attempt loop delegates to `attempt_stream_responses`.
- `attempt_stream_responses` fetches the latest auth, builds headers (including `OpenAI-Beta: responses=experimental`, task kind, optional ChatGPT account), and issues the request through `OtelEventManager::log_request`. Responses are classified into:
  - Success: spawn `process_sse` to consume the SSE stream.
  - Retryable HTTP failures: convert to `StreamAttemptError::RetryableHttpError`, parsing structured JSON bodies for rate-limit metadata and usage-limit errors (which become fatal `CodexErr` variants).
  - Transport errors: wrap as `StreamAttemptError::RetryableTransportError`.
- `process_sse` reads the SSE stream with idle timeout monitoring. It forwards incremental events (`response.output_item.done`, deltas, reasoning updates) as `ResponseEvent`s, synthesises web search begin events, and emits a final `ResponseEvent::Completed` with token usage. It handles fixture files via `stream_from_fixture` for deterministic tests.
- Helper accessors expose provider metadata, reasoning settings, context window limits (by consulting `get_model_info`), and associated `AuthManager`.
- The module exports retry/timeout utilities (`StreamAttemptError::delay`, `parse_rate_limit_snapshot`, `try_parse_retry_after`) alongside an extensive test suite that simulates SSE sequences, error paths, and rate-limit parsing.

## Broader Context
- `ModelClient` is constructed by `codex.rs` for each conversation; CLI/TUI code pipes `ResponseStream` into turn state management and tool orchestration.
- Responses API payloads reuse shared helpers from `client_common` (prompt shaping, tool specs) and `model_provider_info` (wire selection). Chat flows rely on `chat_completions` but still surface through the same `ResponseStream`.
- Telemetry hooks (`OtelEventManager`) unify metric reporting across wire protocols, and rate-limit snapshots inform throttling policy elsewhere in `core`.
- Context can't yet be determined for non-OpenAI providers using Responses; additional wire-compatibility checks may be needed once third-party integrations land.

## Technical Debt
- `process_sse` mixes parsing, retry bookkeeping, and web-search instrumentation; extracting a dedicated parser would simplify reasoning about state transitions and make unit tests more targeted.
- Azure-specific ID patching and verbosity warnings live inline; moving provider-specific quirks behind `ModelProviderInfo` would reduce the branching spread across the request pipeline.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Refactor `process_sse` into a smaller parser/state machine module to reduce the chance of regressions when new SSE event types appear.
    - Push provider-specific payload tweaks (Azure `store: true`, verbosity warnings) into `ModelProviderInfo` so `ModelClient` focuses on request orchestration.
related_specs:
  - ./client_common.rs.spec.md
  - ./chat_completions.rs.spec.md
  - ./model_provider_info.rs.spec.md
  - ./default_client.rs.spec.md
  - ./config.rs.spec.md
