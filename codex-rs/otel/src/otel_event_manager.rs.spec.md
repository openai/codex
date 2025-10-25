## Overview
`otel_event_manager.rs` emits structured telemetry events for Codex interactions, covering conversation lifecycle, API requests, SSE streaming, tool decisions, and user input. It enriches events with metadata such as conversation id, account info, model, and terminal type.

## Detailed Behavior
- `OtelEventMetadata` stores thread-wide context (conversation id, auth mode, account id/email, model slug, logging preferences, app version, terminal type).
- `OtelEventManager` constructors (`new`, `with_model`) initialize metadata and allow updates when the model changes mid-session.
- Event emitters:
  - `conversation_starts` logs configuration details (provider, reasoning effort, sandbox policy, MCP servers, active profile).
  - `log_request` wraps async API calls, emitting `codex.api_request` with duration, status, and errors.
  - `log_sse_event` reports SSE activity, recognizing success markers, output items, and errors/timeouts.
  - `tool_decision` / `tool_decision_result` log decisions, approval sources, and outcomes.
  - `log_user_prompt` optionally records user inputs (honoring `log_user_prompts` flag).
  - `tool_execution_error`, `tool_response_error` capture failures during tool interactions.
  - `request_timeout` logs when API calls exceed allotted time.
- Helpers format timestamps (`timestamp`), errors, and attempt counts for consistent event structures.

## Broader Context
- Consumers use this manager to emit consistent telemetry independent of the logging backend; events flow through `tracing` and, when enabled, through the OTLP exporter configured in `otel_provider.rs`.

## Technical Debt
- None; future events can extend this manager as new telemetry needs arise.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
