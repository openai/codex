# codex-api-client: Proposed Design and Refactor Plan

This document proposes a clearer, smaller, and testable structure for `codex-api-client`, targeting the current pain points:

- `chat.rs` and `responses.rs` are large (600–1100 LOC) and mix multiple concerns.
- SSE parsing, HTTP/retry logic, payload building, and domain event mapping are tangled.
- Azure/ChatGPT quirks live alongside core logic.

The goals here are separation of concerns, shared streaming and retry logic, and focused files that are easy to read and test.

## Overview

- Keep the public API surface compatible: `ApiClient` trait, `ResponsesApiClient`, `ChatCompletionsApiClient`, `ResponseStream`, and `ResponseEvent` remain.
- Internally, split responsibilities into small modules that both clients reuse.
- Centralize SSE framing and retry/backoff, so `chat` and `responses` clients focus only on:
  - payload construction (Prompt → wire payload)
  - mapping wire SSE events → `ResponseEvent`

## Target Module Layout

```
api-client/src/
  api.rs                       # ApiClient trait (unchanged)
  error.rs                     # Error/Result (unchanged interface)
  stream.rs                    # ResponseEvent/ResponseStream (unchanged interface)
  aggregate.rs                 # Aggregation mode (unchanged interface)
  model_provider.rs            # Provider config + headers (unchanged interface)
  routed_client.rs             # Facade routing to Chat/Responses (unchanged interface)

  client/
    mod.rs                     # Re-exports + shared types
    config.rs                  # Common config structs/builders
    http.rs                    # Request building, retries, backoff; returns ByteStream
    rate_limits.rs             # Header parsing → RateLimitSnapshot
    sse.rs                     # Generic SSE line framing + idle-timeout handling
    fixtures.rs                # stream_from_fixture (move from responses.rs)

  payload/
    chat.rs                    # Prompt → Chat Completions JSON
    responses.rs               # Prompt → Responses JSON (+ Azure quirks)
    tools.rs                   # Tool schema conversions and helpers

  decode/
    chat.rs                    # Chat SSE JSON → ResponseEvent (+ function-call state)
    responses.rs               # Responses SSE JSON → ResponseEvent

  clients/
    chat.rs                    # ChatCompletionsApiClient (thin; delegates to payload/http/decode)
    responses.rs               # ResponsesApiClient (thin; delegates to payload/http/decode)
```

Notes
- Modules are organized by responsibility. The `clients/` layer becomes very small.
- `client/http.rs` owns retries/backoff, request building, headers, and returns a `Stream<Item = Result<Bytes>>`.
- `client/sse.rs` owns SSE framing and idle-timeout. It surfaces framed JSON strings to decoders.
- `decode/*` mappers transform framed JSON into `ResponseEvent` using only parsing/state.
- `payload/*` generate request JSON. Azure and tool-shape specifics live here.
- `client/rate_limits.rs` parses headers and emits a `ResponseEvent::RateLimits` once, near stream start.
- `client/fixtures.rs` provides the file-backed stream used by tests and local dev.

## Trait-Based Core

Introduce small traits for payload construction and decoding to maximize reuse and make the concrete Chat/Responses clients thin bindings.

- `PayloadBuilder`
  - `fn build(&self, prompt: &Prompt) -> Result<serde_json::Value>`
  - Implementations: `payload::chat::Builder`, `payload::responses::Builder`.

- `ResponseDecoder`
  - Consumes framed SSE JSON and emits `ResponseEvent`s.
  - Suggested interface:
    - `fn on_frame(&mut self, json: &str, tx: &mpsc::Sender<Result<ResponseEvent>>, otel: &OtelEventManager) -> Result<()>`
    - Implementations: `decode::chat::Decoder`, `decode::responses::Decoder`.

- Optional adapters
  - `RateLimitProvider`: `fn parse(&self, headers: &HeaderMap) -> Option<RateLimitSnapshot>`
  - `RequestCustomizer`: per-API header tweaks (e.g., Conversations/Session headers for Responses).

With these traits, a generic client wrapper can stitch components together:

```rust
struct GenericClient<PB, DEC> {
  http: RequestExecutor,
  payload: PB,
  decoder: DEC,
  idle: Duration,
  otel: OtelEventManager,
}

impl<PB: PayloadBuilder, DEC: ResponseDecoder> GenericClient<PB, DEC> {
  async fn stream(&self, prompt: &Prompt) -> Result<ResponseStream> {
    let payload = self.payload.build(prompt)?;
    let (headers, bytes) = self.http.execute_stream(payload, prompt).await?;
    if let Some(snapshot) = rate_limits::parse(&headers) { /* emit event */ }
    let sse_stream = sse::frame(bytes, self.idle, self.otel.clone());
    // spawn: for each framed JSON chunk → self.decoder.on_frame(...)
    /* return ResponseStream */
  }
}
```

Chat/Responses become type aliases or thin wrappers around `GenericClient` with the appropriate `PayloadBuilder` and `ResponseDecoder`.

## Responsibility Boundaries

- Clients (`clients/chat.rs`, `clients/responses.rs`)
  - Validate prompt constraints (e.g., Chat lacks `output_schema`).
  - Build payload via `payload::*`.
  - Build and send request via `client/http.rs`.
  - Create an SSE pipeline: `http::stream(...) → sse::frame(...) → decode::<api>::map(...)`.
  - Forward `ResponseEvent`s to the `mpsc` channel.

- HTTP (`client/http.rs`)
  - `RequestExecutor::execute_stream(req: RequestSpec) -> Result<(Headers, ByteStream)>`.
  - Injects auth/session headers and provider headers via `ModelProviderInfo`.
  - Centralized retry policy for non-2xx, 429, 401, 5xx, and transport errors.
  - Handles `Retry-After` and exponential backoff (`backoff()`).
  - Returns first successful response’s headers and stream; does not parse SSE.

- SSE (`client/sse.rs`)
  - Takes a `Stream<Item = Result<Bytes>>` and produces framed JSON strings by handling `data:` lines and chunk boundaries.
  - Enforces idle timeout and signals early stream termination errors.
  - Does no schema parsing; just a robust line/framing codec.

- Decoders (`decode/chat.rs`, `decode/responses.rs`)
  - Take framed JSON string(s) and emit `ResponseEvent`s.
  - Own API-specific state machines: e.g., Chat function-call accumulation; Responses “event-shaped” and “field-shaped” variants.
  - No networking, no backoff, no channels.

- Payload builders (`payload/chat.rs`, `payload/responses.rs`, `payload/tools.rs`)
  - Convert `Prompt` to provider-specific JSON (Chat/Responses). Keep pure and deterministic.
  - Azure-specific adjustments (e.g., attach item IDs) live here.

- Rate limits (`client/rate_limits.rs`)
  - Parse headers to `RateLimitSnapshot`.
  - Emit a single `ResponseEvent::RateLimits` at stream start when present.

## Stream Pipeline

```
ByteStream (reqwest) → sse::frame (idle timeout, data: framing) → decode::<api> → ResponseEvent
```

Pseudocode for both clients:

```rust
let (headers, byte_stream) = http.execute_stream(request_spec).await?;
if let Some(snapshot) = rate_limits::parse(&headers) {
    tx.send(Ok(ResponseEvent::RateLimits(snapshot))).await.ok();
}
let sse_stream = sse::frame(byte_stream, idle_timeout, otel.clone());
tokio::spawn(decode::<Api>::run(sse_stream, tx.clone(), otel.clone()));
Ok(ResponseStream { rx_event })
```

Where `decode::<Api>::run` is API-specific mapping of framed JSON into `ResponseEvent`s.

## Incremental Refactor Plan

Do this in small, safe steps. Public API stays stable at each step.

0) Introduce traits
- Add `PayloadBuilder` and `ResponseDecoder` traits.
- Provide initial implementations backed by existing code paths to minimize churn.

1) Extract shared helpers
- Move rate-limit parsing from `responses.rs` to `client/rate_limits.rs`.
- Move `stream_from_fixture` to `client/fixtures.rs`.
- Keep old re-exports from `lib.rs` to avoid churn.

2) Isolate SSE framing
- Extract line framing + idle-timeout from `responses.rs::process_sse` into `client/sse.rs`.
- Have `responses.rs` use `sse::frame` and keep its own JSON mapping for now.

3) Centralize HTTP execution
- Create `client/http.rs` with `RequestExecutor` handling retries/backoff and returning `(headers, stream)`.
- Switch `responses.rs` to use it.
- Align Chat client to use `RequestExecutor` as well.

4) Split JSON mapping into decoders
- Move JSON → `ResponseEvent` mapping from `responses.rs` to `decode/responses.rs`.
- Do the same for Chat (`chat.rs` → `decode/chat.rs`).

5) Extract payload builders
- Move payload JSON construction into `payload/chat.rs` and `payload/responses.rs`.
- Move tool helpers into `payload/tools.rs`.

6) Thin the clients
- Create `clients/chat.rs` and `clients/responses.rs` that glue together payload → http → sse → decode.
- Keep existing type names and `impl ApiClient` blocks; only relocate logic behind them.

7) Clean-up and local boundaries
- Remove now-unused code paths from the original large files.
- Ensure `mod` declarations reflect the new module structure.

8) Tests and validation
- Unit-test `sse::frame` against split and concatenated `data:` lines.
- Unit-test both decoders with small fixtures for typical and edge cases.
- Unit-test payload builders on prompts containing messages, images, tools, and reasoning.
- Keep existing integration tests using `stream_from_fixture`.

## File Size Targets (post-refactor)

- `clients/chat.rs`: ~100–150 LOC
- `clients/responses.rs`: ~150–200 LOC
- `decode/chat.rs`: ~200–250 LOC (function-call state lives here)
- `decode/responses.rs`: ~250–300 LOC (event/field-shaped mapping)
- `client/http.rs`: ~150–200 LOC (shared retries)
- `client/sse.rs`: ~120–160 LOC (framing + timeout)
- `payload/chat.rs`: ~120–180 LOC
- `payload/responses.rs`: ~120–160 LOC

## Error Handling and Retries

- Single retry policy in `client/http.rs`:
  - Retry 429/401/5xx with `Retry-After` when present or with exponential backoff.
  - Transport errors (DNS/reset/timeouts) are retryable up to provider-configured attempts.
  - Non-retryable statuses return `UnexpectedStatus` with body for diagnosis.
- `decode/*` surface protocol-specific “quota/context window exceeded” errors as stable messages already recognized by callers.

## Instrumentation

- `sse::frame` triggers idle-timeout failures and marks event kinds only when actual JSON events appear; decoders record specific kinds (e.g., `response.completed`).
- `http::execute_stream` wraps the request with `otel_event_manager.log_request(...)` and populates `request_id` when applicable.

## Azure and ChatGPT Specifics

- Keep all Azure id attachment logic in `payload/responses.rs`.
- Keep ChatGPT auth header handling in `http.rs` via `AuthProvider` (unchanged trait), based on `RequestSpec`’s context.

## Configuration

Optionally introduce typed builders for client configs in `client/config.rs` to reduce parameter plumbing and make defaults explicit:

```rust
ResponsesConfig::builder()
  .provider(provider)
  .model(model)
  .conversation_id(conv_id)
  .otel(otel)
  .auth_provider(auth)
  .build();
```

Builder is additive; existing constructors remain.

## Backpressure and Channels

- Keep channel capacity at 1600 (as today) but make it a constant inside `clients/*` so we can tune independently per client.
- Decoders emit `OutputItemAdded` before subsequent deltas for the same item when required by downstream consumers.

## Migration Notes

- Public re-exports in `lib.rs` remain stable.
- Module moves are internal; no external callers need to change imports.
- When moving functions, preserve names and signatures where feasible to minimize diff churn.

## Acceptance Criteria

- Both Chat and Responses clients reduce to thin orchestration files.
- SSE framing, retries, and rate-limit parsing exist exactly once and are used by both clients.
- All behavior remains functionally equivalent (or better tested) after refactor.
- New unit tests cover framing, decoders, and payload builders.

## Open Questions

- Should `aggregate.rs` own more of the delta → aggregated assembly, now that both decoders emit the same `ResponseEvent` kinds? For this iteration, keep as-is.
- Should we expose a single unified `Client` that auto-selects Chat/Responses by provider? We already have `routed_client`; keep it stable and thin it later using the new internals.
- Do we want to expose backoff policy knobs at runtime? For now, keep provider-driven.

---

This plan preserves the external API while making internals smaller, reusable, and easier to test. It can be applied incrementally with meaningful checkpoints and test coverage increases at each step.
