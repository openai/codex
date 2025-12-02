# codex-api Port to Kotlin

This document tracks the progress of porting `codex-rs/codex-api` to Kotlin Multiplatform Native under `src/nativeMain/kotlin/ai/solace/coder/api`.

## Port Status

### ✅ Completed Modules

#### auth (codex-api/src/auth.rs)
- **Location**: `ai.solace.coder.api.auth`
- **Files**: 
  - `AuthProvider.kt` - Interface for bearer token and account ID provisioning
  - `AuthHeaders.kt` - `addAuthHeaders()` function using Ktor `HttpRequestBuilder`
- **Notes**: Fully ported, uses Ktor for HTTP integration

#### error (codex-api/src/error.rs)
- **Location**: `ai.solace.coder.api.error`
- **Files**: `ApiError.kt`
- **Status**: Sealed class with all error cases (Transport, Api, Stream, ContextWindowExceeded, QuotaExceeded, UsageNotIncluded, Retryable, RateLimit)
- **TODOs**: Replace status `Int` with proper StatusCode type once ported

#### provider (codex-api/src/provider.rs)
- **Location**: `ai.solace.coder.api.provider`
- **Files**: `Provider.kt`
- **Status**: 
  - `WireApi` enum (Responses, Chat, Compact)
  - `RetryConfig` data class
  - `Provider` data class with `urlForPath()`, `buildRequest()`, `isAzureResponsesEndpoint()`
- **Notes**: Uses Ktor `HttpRequestBuilder` for request construction

#### common (codex-api/src/common.rs)
- **Location**: `ai.solace.coder.api.common`
- **Files**: `Common.kt`
- **Status**: 
  - `Prompt`, `CompactionInput`, `ResponseEvent` (sealed class)
  - `Reasoning`, `TextFormat`, `TextControls`, `OpenAiVerbosity`
  - `ResponsesApiRequest`, `createTextParamForRequest()`
  - `ResponseStream` interface
- **TODOs**: 
  - Replace placeholder types (ResponseItem, TokenUsage, RateLimitSnapshot, etc.) once codex-protocol is ported
  - Implement proper channel/Flow for ResponseStream

#### requests (codex-api/src/requests/*.rs)
- **Location**: `ai.solace.coder.api.requests`
- **Files**:
  - `Headers.kt` - Internal header helpers (`buildConversationHeaders`, `subagentHeader`, `insertHeader`)
  - `ChatRequest.kt` - `ChatRequest` and `ChatRequestBuilder`
  - `ResponsesRequest.kt` - `ResponsesRequest` and `ResponsesRequestBuilder`
- **Status**: Basic structure ported with kotlinx.serialization.json for payloads
- **TODOs**: 
  - Full message processing logic (reasoning anchoring, deduplication) in ChatRequestBuilder
  - Proper ResponseItem type from codex-protocol
  - SessionSource and SubAgentSource porting
  - Azure ID attachment logic in ResponsesRequestBuilder

#### telemetry (codex-api/src/telemetry.rs)
- **Location**: `ai.solace.coder.api.telemetry`
- **Files**: `Telemetry.kt`
- **Status**: 
  - `SseTelemetry` interface
  - `RequestTelemetry` interface
  - `runWithRequestTelemetry()` helper
- **TODOs**: Full retry policy integration with Ktor

#### rate_limits (codex-api/src/rate_limits.rs)
- **Location**: `ai.solace.coder.api.ratelimits`
- **Files**: `RateLimits.kt`
- **Status**: 
  - `RateLimitError`, `parseRateLimit()` with header parsing
  - `RateLimitSnapshot`, `RateLimitWindow`, `CreditsSnapshot` data classes
- **Notes**: Fully functional, parses Codex-specific rate limit headers from Ktor `Headers`

#### endpoint (codex-api/src/endpoint/*.rs)
- **Location**: `ai.solace.coder.api.endpoint`
- **Files**:
  - `StreamingClient.kt` - Internal streaming client with auth and telemetry
  - `ChatClient.kt` - Chat completions endpoint client
  - `ResponsesClient.kt` - Responses endpoint client with `ResponsesOptions`
  - `CompactClient.kt` - Compaction endpoint client
- **Status**: Structure ported, methods present
- **TODOs**:
  - Implement SSE spawning (`spawnChatStream`, `spawnResponsesStream`)
  - Wire up retry policy with telemetry
  - Implement `CompactClient.compact()` with POST and JSON parsing

#### sse (codex-api/src/sse/*.rs)
- **Location**: `ai.solace.coder.api.sse`
- **Files**: `SSE.kt`
- **Status**: Stub functions created
- **TODOs**:
  - `spawnChatStream()` - Parse SSE events into ResponseEvent stream
  - `spawnResponsesStream()` - Parse SSE events into ResponseEvent stream
  - `streamFromFixture()` - Test fixture loading
  - `parseResponseEvent()` - JSON event parsing
  - Integrate eventsource-stream equivalent (Ktor SSE support or custom parser)

### 📦 External Dependencies

The Kotlin port uses:
- **Ktor Client** (`io.ktor.client.*`) for HTTP requests and streaming
- **kotlinx.serialization.json** for JSON payloads
- **kotlin.time** for Duration and timing measurements

### 🚧 Missing Integrations

1. **codex-protocol types**: ResponseItem, TokenUsage, RateLimitSnapshot, SessionSource, SubAgentSource, ContentItem, etc.
2. **SSE/EventSource parsing**: Need Ktor SSE client plugin or custom parser
3. **Retry policy**: Full exponential backoff with transport/HTTP error detection
4. **JSON Value type**: Currently using `Any` or `JsonElement` as placeholders

### 📝 Package Structure

```
ai.solace.coder.api/
├── auth/
│   ├── AuthProvider.kt
│   └── AuthHeaders.kt
├── common/
│   └── Common.kt
├── endpoint/
│   ├── ChatClient.kt
│   ├── CompactClient.kt
│   ├── ResponsesClient.kt
│   └── StreamingClient.kt
├── error/
│   └── ApiError.kt
├── provider/
│   └── Provider.kt
├── ratelimits/
│   └── RateLimits.kt
├── requests/
│   ├── ChatRequest.kt
│   ├── Headers.kt
│   └── ResponsesRequest.kt
├── sse/
│   └── SSE.kt
├── telemetry/
│   └── Telemetry.kt
└── lib.kt (placeholder for re-exports)
```

### 🎯 Next Steps

1. Port codex-protocol types (models, protocol enums/structs)
2. Implement SSE parsing with Ktor SSE plugin or custom EventSource parser
3. Wire up ChatClient and ResponsesClient with actual SSE spawners
4. Implement retry policy with exponential backoff
5. Complete CompactClient POST implementation
6. Add unit tests mirroring Rust test suite
7. Update `lib.kt` with public API re-exports

### 🔗 Cross-References

- **AuthManager** remains in `ai.solace.coder.client.auth` (from codex-core)
- This `ai.solace.coder.api` package is a clean port of codex-api crate only
- No cross-crate consolidation; API boundaries preserved

