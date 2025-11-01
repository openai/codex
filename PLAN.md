Title: Delegate /v1/responses over JSON‑RPC in app‑server

Overview

Add an opt‑in transport so Codex core (running inside app‑server) delegates the actual HTTP POST to the OpenAI Responses API to the JSON‑RPC client. The app‑server sends a server→client JSON‑RPC request carrying the full request body and headers; the client performs the HTTP call, streams SSE events back as client→server notifications, and finally replies to the original JSON‑RPC request with a terminal result.

Scope

- Protocol: add a ServerRequest for starting a Responses call and a ClientNotification for streaming SSE events.
- Core: add a small delegate hook that, when enabled, replaces the direct reqwest path in ModelClient::attempt_stream_responses. Provide an event processor that maps raw Responses events (JSON envelopes) into internal ResponseEvent, mirroring the existing SSE mapper.
- App‑server: register a delegate implementation that uses OutgoingMessageSender to send Requests, tracks call_id→event channel mappings, and forwards incoming ClientNotifications to core.
- Tests: end‑to‑end app‑server test exercising the new path by simulating a client that responds to the server’s request with a stream of Responses events.

Protocol changes

- ServerRequest variant: responsesApi/call
  - Params: { conversationId, callId, url, headers: {k: v}, body: json, stream: bool }
  - Response: { status: u16, requestId?: string, usage?: TokenUsage, error?: string }
- ClientNotification variant: responsesApi/event
  - Params: { callId: string, event: json } (raw Responses API event JSON)

Core changes

- Add core::responses_delegate module with:
  - Trait ResponsesHttpDelegate { start_call(call_id, params) -> Future<Result<()>>; incoming_event(call_id, event_json) }
  - Global registration (OnceLock) and helpers to register and to deliver incoming events.
- Modify ModelClient::attempt_stream_responses to branch when:
  - Wire API is Responses, feature toggle is enabled, and a delegate is registered.
  - Generate a call_id, create an mpsc channel for raw event JSON, register it, and spawn a processor to map JSON events to ResponseEvent.
  - Invoke delegate.start_call with the request (url, headers, payload) and return a ResponseStream tied to the mapped event channel.
- Implement a JSON event mapper mirroring the existing Responses SSE event handling (response.created, response.output_item.done, response.output_text.delta, response.reasoning_* deltas, response.failed, response.completed).

App‑server changes

- Implement AppServerResponsesDelegate that:
  - Keeps an Arc<OutgoingMessageSender> and a map call_id→mpsc::Sender<serde_json::Value>.
  - Implements start_call by sending ServerRequest::responsesApi/call and awaiting the JSON‑RPC response in a task.
  - Implements incoming_event by looking up call_id and forwarding the event JSON to the registered sender.
- Wire registration at startup in run_main when feature toggle is enabled in config.
- Route client notifications in MessageProcessor::process_notification: parse to ClientNotification and forward responsesApi/event to core::responses_delegate::incoming_event.

Feature toggle

- Add a Features flag key: responses_http_over_jsonrpc (default false). Core checks this to decide whether to delegate.

Tests

- New integration test under codex-rs/app-server/tests/suite:
  - Enable the feature via config.toml ([features] responses_http_over_jsonrpc = true).
  - Start app‑server (McpProcess); create a conversation; add listener with experimental_raw_events = true.
  - Send a user message; expect a ServerRequest::responsesApi/call; stream responsesApi/event notifications back (response.created, response.output_item.done (assistant text), response.completed); reply to the server request with a 200 JSON‑RPC response.
  - Assert the sendUserMessage call completes and raw_response_item notifications include the assistant message.

Out of scope

- Client‑side implementation beyond tests (VS Code extension or SDK) — the protocol is additive.
- Persisting request metrics in the final JSON‑RPC response; core relies on streamed events (response.completed) for usage.

Rollout / Compatibility

- Fully opt‑in behind the features.responses_http_over_jsonrpc flag.
- Fallback to existing HTTP path when the feature is disabled or no delegate is registered.

