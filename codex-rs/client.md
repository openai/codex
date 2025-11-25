# Client Extraction Plan

## Goals
- Split the HTTP transport/client code out of `codex-core` into a reusable crate that is agnostic of Codex/OpenAI business logic and API schemas.
- Create a separate API library crate that houses typed requests/responses for well-known APIs (Responses, Chat Completions, Compact) and plugs into the transport crate via minimal traits.
- Preserve current behaviour (auth headers, retries, SSE handling, rate-limit parsing, compaction, fixtures) while making the APIs symmetric and avoiding code duplication.
- Keep existing consumers (`codex-core`, tests, and tools) stable by providing a small compatibility layer during the transition.

## Snapshot of Today
- `core/src/client.rs (ModelClient)` owns config/auth/session state, chooses wire API, builds payloads, drives retries, parses SSE, compaction, and rate-limit headers.
- `core/src/chat_completions.rs` implements the Chat Completions call + SSE parser + aggregation helper.
- `core/src/client_common.rs` holds `Prompt`, tool specs, shared request structs (`ResponsesApiRequest`, `TextControls`), and `ResponseEvent`/`ResponseStream`.
- `core/src/default_client.rs` wraps `reqwest` with Codex UA/originator defaults.
- `core/src/model_provider_info.rs` models providers (base URL, headers, env keys, retry/timeout tuning) and builds `CodexRequestBuilder`s.
    - Current retry logic is co-located with API handling; streaming SSE parsing is duplicated across Responses/Chat.

## Target Crates (with interfaces)

- `codex-client` (generic transport)
  - Owns the generic HTTP machinery: a `CodexHttpClient`/`CodexRequestBuilder`-style wrapper, retry/backoff hooks, streaming connector (SSE framing + idle timeout), header injection, and optional telemetry callbacks.
  - Does **not** know about OpenAI/Codex-specific paths, headers, or error codes; it only exposes HTTP-level concepts (status, headers, bodies, connection errors).
  - Minimal surface:
    ```rust
    pub trait HttpTransport {
        fn execute(&self, req: Request) -> Result<Response, TransportError>;
        fn stream(&self, req: Request) -> Result<ByteStream, TransportError>;
    }

    pub struct Request {
        pub method: Method,
        pub url: String,
        pub headers: HeaderMap,
        pub body: Option<serde_json::Value>,
        pub timeout: Option<Duration>,
    }
    ```
  - Generic client traits (request/response/chunk are abstract over the transport):
    ```rust
    #[async_trait::async_trait]
    pub trait UnaryClient<Req, Resp> {
        async fn run(&self, req: Req) -> Result<Resp, TransportError>;
    }

    #[async_trait::async_trait]
    pub trait StreamClient<Req, Chunk> {
        async fn run(&self, req: Req) -> Result<ResponseStream<Chunk>, TransportError>;
    }

    pub struct RetryPolicy {
        pub max_attempts: u64,
        pub base_delay: Duration,
        pub retry_on: RetryOn, // e.g., transport errors + 429/5xx
    }
    ```
    - `RetryOn` lives in `codex-client` and captures HTTP status classes and transport failures that qualify for retry.
    - Implementations in `codex-api` plug in their own request types, parsers, and retry policies while reusing the transport’s backoff and error types.
    - Planned runtime helper:
      ```rust
      pub async fn run_with_retry<T, F, Fut>(
          policy: RetryPolicy,
          make_req: impl Fn() -> Request,
          op: F,
      ) -> Result<T, TransportError>
      where
          F: Fn(Request) -> Fut,
          Fut: Future<Output = Result<T, TransportError>>,
      {
          for attempt in 0..=policy.max_attempts {
              let req = make_req();
              match op(req).await {
                  Ok(resp) => return Ok(resp),
                  Err(err) if policy.retry_on.should_retry(&err, attempt) => {
                      tokio::time::sleep(backoff(policy.base_delay, attempt + 1)).await;
                  }
                  Err(err) => return Err(err),
              }
          }
          Err(TransportError::RetryLimit)
      }
      ```
      - Unary clients wrap `transport.execute` with this helper and then deserialize.
      - Stream clients wrap the **initial** `transport.stream` call with this helper. Mid-stream disconnects are surfaced as `StreamError`s; automatic resume/reconnect can be added later on top of this primitive if we introduce cursor support.
  - Common helpers: `retry::backoff(attempt)`, `errors::{TransportError, StreamError}`.
  - Streaming utility (SSE framing only):
    ```rust
    pub fn sse_stream<S>(
        bytes: S,
        idle_timeout: Duration,
        tx: mpsc::Sender<Result<String, StreamError>>,
        telemetry: Option<Box<dyn Telemetry>>,
    )
    where
        S: Stream<Item = Result<Bytes, TransportError>> + Unpin + Send + 'static;
    ```
    - `sse_stream` is responsible for timeouts, connection-level errors, and emitting raw `data:` chunks as UTF-8 strings; parsing those strings into structured events is done in `codex-api`.

- `codex-api` (OpenAI/Codex API library)
  - Owns typed models for Responses/Chat/Compact plus shared helpers (`Prompt`, tool specs, text controls, `ResponsesApiRequest`, etc.).
  - Knows about OpenAI/Codex semantics:
    - URL shapes (`/v1/responses`, `/v1/chat/completions`, `/responses/compact`).
    - Provider configuration (`WireApi`, base URLs, query params, per-provider retry knobs).
    - Rate-limit headers (`x-codex-*`) and their mapping into `RateLimitSnapshot` / `CreditsSnapshot`.
    - Error body formats (`{ error: { type, code, message, plan_type, resets_at } }`) and how they become API errors (context window exceeded, quota/usage limit, etc.).
    - SSE event names (`response.output_item.done`, `response.completed`, `response.failed`, etc.) and their mapping into high-level events.
  - Provides a provider abstraction (conceptually similar to `ModelProviderInfo`):
    ```rust
    pub struct Provider {
        pub name: String,
        pub base_url: String,
        pub wire: WireApi, // Responses | Chat
        pub headers: HeaderMap,
        pub retry: RetryConfig,
        pub stream_idle_timeout: Duration,
    }

    pub trait AuthProvider {
        /// Returns a bearer token to use for this request (if any).
        /// Implementations are expected to be cheap and to surface already-refreshed tokens;
        /// higher layers (`codex-core`) remain responsible for token refresh flows.
        fn bearer_token(&self) -> Option<String>;

        /// Optional ChatGPT account id header for Chat mode.
        fn account_id(&self) -> Option<String>;
    }
    ```
  - Ready-made clients built on `HttpTransport`:
    ```rust
    pub struct ResponsesClient<T: HttpTransport, A: AuthProvider> { /* ... */ }
    impl<T, A> ResponsesClient<T, A> {
        pub async fn stream(&self, prompt: &Prompt) -> ApiResult<ResponseStream<ApiEvent>>;
        pub async fn compact(&self, prompt: &Prompt) -> ApiResult<Vec<ResponseItem>>;
    }

    pub struct ChatClient<T: HttpTransport, A: AuthProvider> { /* ... */ }
    impl<T, A> ChatClient<T, A> {
        pub async fn stream(&self, prompt: &Prompt) -> ApiResult<ResponseStream<ApiEvent>>;
    }

    pub struct CompactClient<T: HttpTransport, A: AuthProvider> { /* ... */ }
    impl<T, A> CompactClient<T, A> {
        pub async fn compact(&self, prompt: &Prompt) -> ApiResult<Vec<ResponseItem>>;
    }
    ```
  - Streaming events unified across wire APIs (this can closely mirror `ResponseEvent` today, and we may type-alias one to the other during migration):
    ```rust
    pub enum ApiEvent {
        Created,
        OutputItemAdded(ResponseItem),
        OutputItemDone(ResponseItem),
        OutputTextDelta(String),
        ReasoningContentDelta { delta: String, content_index: i64 },
        ReasoningSummaryDelta { delta: String, summary_index: i64 },
        RateLimits(RateLimitSnapshot),
        Completed { response_id: String, token_usage: Option<TokenUsage> },
    }
    ```
  - Error layering:
    - `codex-client`: defines `TransportError` / `StreamError` (status codes, IO, timeouts).
    - `codex-api`: defines `ApiError` that wraps `TransportError` plus API-specific errors parsed from bodies and headers.
    - `codex-core`: maps `ApiError` into existing `CodexErr` variants so downstream callers remain unchanged.
  - Aggregation strategies (today’s `AggregateStreamExt`) live here as adapters (`Aggregated`, `Streaming`) that transform `ResponseStream<ApiEvent>` into the higher-level views used by `codex-core`.

## Implementation Steps

1. **Create crates**: add `codex-client` and `codex-api` (names keep the `codex-` prefix). Stub lib files with feature flags/tests wired into the workspace; wire them into `Cargo.toml`.
2. **Extract API-level SSE + rate limits into `codex-api`**:
   - Move the Responses SSE parser (`process_sse`), rate-limit parsing, and related tests from `core/src/client.rs` into `codex-api`, keeping the behavior identical.
   - Introduce `ApiEvent` (initially equivalent to `ResponseEvent`) and `ApiError`, and adjust the parser to emit those.
   - Provide test-only helpers for fixture streams (replacement for `CODEX_RS_SSE_FIXTURE`) in `codex-api`.
3. **Lift transport layer into `codex-client`**:
   - Move `CodexHttpClient`/`CodexRequestBuilder`, UA/originator plumbing, and backoff helpers from `core/src/default_client.rs` into `codex-client` (or a thin wrapper on top of it).
   - Introduce `HttpTransport`, `Request`, `RetryPolicy`, `RetryOn`, and `run_with_retry` as described above.
   - Keep sandbox/no-proxy toggles behind injected configuration so `codex-client` stays generic and does not depend on Codex-specific env vars.
4. **Model provider abstraction in `codex-api`**:
   - Relocate `ModelProviderInfo` (base URL, env/header resolution, retry knobs, wire API enum) into `codex-api`, expressed in terms of `Provider` and `AuthProvider`.
   - Ensure provider logic handles:
     - URL building for Responses/Chat/Compact (including Azure special cases).
     - Static and env-based headers.
     - Per-provider retry and idle-timeout settings that map cleanly into `RetryPolicy`/`RetryOn`.
5. **API crate wiring**:
   - Move `Prompt`, tool specs, `ResponsesApiRequest`, `TextControls`, and `ResponseEvent/ResponseStream` into `codex-api` under modules (`common`, `responses`, `chat`, `compact`), keeping public types stable or re-exported through `codex-core` as needed.
   - Rebuild Responses and Chat clients on top of `HttpTransport` + `StreamClient`, reusing shared retry + SSE helpers; keep aggregation adapters as reusable strategies instead of `ModelClient`-local logic.
   - Implement Compact on top of `UnaryClient` and the unary `execute` path with JSON deserialization, sharing the same retry policy.
   - Keep request builders symmetric: each client prepares a `Request<serde_json::Value>`, attaches headers/auth via `AuthProvider`, and plugs in its parser (streaming clients) or deserializer (unary) while sharing retry/backoff configuration derived from `Provider`.
6. **Core integration layer**:
   - Replace `core::ModelClient` internals with thin adapters that construct `codex-api` clients using `Config`, `AuthManager`, and `OtelEventManager`.
   - Keep the public `ModelClient` API and `ResponseEvent`/`ResponseStream` types stable by re-exporting `codex-api` types or providing type aliases.
   - Preserve existing auth flows (including ChatGPT token refresh) inside `codex-core` or a thin adapter, using `AuthProvider` to surface bearer tokens to `codex-api` and handling 401/refresh semantics at this layer.
7. **Tests/migration**:
   - Move unit tests for SSE parsing, retry/backoff decisions, and provider/header behavior into the new crates; keep integration tests in `core` using the compatibility layer.
   - Update fixtures to be consumed via test-only adapters in `codex-api`.
   - Run targeted `just fmt`, `just fix -p` for the touched crates, and scoped `cargo test -p codex-client`, `-p codex-api`, and existing `codex-core` suites.

## Design Decisions

- **UA construction**
  - `codex-client` exposes an optional UA suffix/provider hook (tiny feature) and remains unaware of the CLI; `codex-core` / the CLI compute the full UA (including `terminal::user_agent()`) and pass the suffix or builder down.
- **Config vs provider**
  - Most configuration stays in `codex-core`. `codex-api::Provider` only contains what is strictly required for HTTP (base URLs, query params, retry/timeout knobs, wire API), while higher-level knobs (reasoning defaults, verbosity flags, etc.) remain core concerns.
- **Auth flow ownership**
  - Auth flows (including ChatGPT token refresh) remain in `codex-core`. `AuthProvider` simply exposes already-fresh tokens/account IDs; 401 handling and refresh retries stay in the existing auth layer.
- **Error enums**
  - `codex-client` continues to define `TransportError` / `StreamError`. `codex-api` defines an `ApiError` (deriving `thiserror::Error`) that wraps `TransportError` and API-specific failures, and `codex-core` maps `ApiError` into existing `CodexErr` variants for callers.
- **Streaming reconnection semantics**
  - For now, mid-stream SSE failures are surfaced as errors and only the initial connection is retried via `run_with_retry`. We will revisit mid-stream reconnect/resume once the underlying APIs support cursor/idempotent event semantics.

