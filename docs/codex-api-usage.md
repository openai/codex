# codex-api Usage Examples

This document shows how to use the newly ported `ai.solace.coder.api` package.

## Basic Setup

```kotlin
import ai.solace.coder.api.auth.AuthProvider
import ai.solace.coder.api.provider.*
import ai.solace.coder.api.endpoint.*
import io.ktor.client.*
import kotlin.time.Duration.Companion.seconds

// Implement AuthProvider
class MyAuthProvider(private val token: String) : AuthProvider {
    override fun bearerToken(): String = token
    override fun accountId(): String? = null
}

// Create a provider
val provider = Provider(
    name = "openai",
    baseUrl = "https://api.openai.com/v1",
    queryParams = null,
    wire = WireApi.Chat,
    defaultHeaders = mapOf("User-Agent" to "codex-kotlin/0.1.0"),
    retry = RetryConfig(
        maxAttempts = 3,
        baseDelay = 100.milliseconds,
        retry429 = true,
        retry5xx = true,
        retryTransport = true,
    ),
    streamIdleTimeout = 30.seconds,
)

// Create HTTP client
val httpClient = HttpClient()

// Create auth
val auth = MyAuthProvider("sk-...")
```

## Using ChatClient

```kotlin
import ai.solace.coder.api.endpoint.ChatClient
import ai.solace.coder.api.requests.ChatRequestBuilder

val chatClient = ChatClient(httpClient, provider, auth)

// Build a request
val request = ChatRequestBuilder(
    model = "gpt-4",
    instructions = "You are a helpful assistant.",
    input = listOf(/* ResponseItems */),
    tools = emptyList(),
)
    .conversationId("conv-123")
    .build(provider)
    .getOrThrow()

// Stream the response (once SSE is implemented)
// val stream = chatClient.streamRequest(request).getOrThrow()
// stream.collect { event ->
//     println("Event: $event")
// }
```

## Using ResponsesClient

```kotlin
import ai.solace.coder.api.endpoint.ResponsesClient
import ai.solace.coder.api.endpoint.ResponsesOptions
import ai.solace.coder.api.requests.ResponsesRequestBuilder

val responsesClient = ResponsesClient(httpClient, provider, auth)

// Build a request
val request = ResponsesRequestBuilder(
    model = "gpt-4",
    instructions = "You are a helpful assistant.",
    input = listOf(/* ResponseItemProtocol */),
)
    .conversation("conv-123")
    .include(listOf("output_item_done"))
    .build(provider)
    .getOrThrow()

// Stream the response (once SSE is implemented)
// val stream = responsesClient.streamRequest(request).getOrThrow()
```

## Using Provider Utilities

```kotlin
import ai.solace.coder.api.provider.Provider

val provider = Provider(/* ... */)

// Build URLs
val url = provider.urlForPath("chat/completions")
// Result: "https://api.openai.com/v1/chat/completions"

// Build HTTP requests
val requestBuilder = provider.buildRequest(HttpMethod.Post, "responses") {
    // Additional configuration
}

// Check if Azure endpoint
if (provider.isAzureResponsesEndpoint()) {
    println("Using Azure")
}
```

## Rate Limit Parsing

```kotlin
import ai.solace.coder.api.ratelimits.parseRateLimit
import io.ktor.client.statement.*

val response: HttpResponse = /* ... */
val rateLimit = parseRateLimit(response.headers)

rateLimit?.primary?.let { window ->
    println("Primary rate limit: ${window.usedPercent}% used")
}
```

## Adding Auth Headers

```kotlin
import ai.solace.coder.api.auth.addAuthHeaders
import io.ktor.client.request.*

val auth = MyAuthProvider("sk-...")

val request = HttpRequestBuilder().apply {
    url("https://api.openai.com/v1/chat/completions")
    addAuthHeaders(auth, this)
    // Headers now include:
    // Authorization: Bearer sk-...
}
```

## Error Handling

```kotlin
import ai.solace.coder.api.error.ApiError

val result = chatClient.streamRequest(request)

result.fold(
    onSuccess = { stream ->
        // Handle stream
    },
    onFailure = { error ->
        when (error) {
            is ApiError.Transport -> println("Transport error: ${error.message}")
            is ApiError.Api -> println("API error ${error.status}: ${error.message}")
            is ApiError.RateLimit -> println("Rate limited: ${error.message}")
            is ApiError.ContextWindowExceeded -> println("Context window exceeded")
            else -> println("Unknown error: $error")
        }
    }
)
```

## Telemetry Integration

```kotlin
import ai.solace.coder.api.telemetry.RequestTelemetry
import ai.solace.coder.api.telemetry.SseTelemetry
import io.ktor.http.*
import kotlin.time.Duration

class MyTelemetry : RequestTelemetry, SseTelemetry {
    override fun onRequest(
        attempt: Int,
        status: HttpStatusCode?,
        error: Throwable?,
        duration: Duration
    ) {
        println("Request attempt $attempt: status=$status, duration=$duration")
    }
    
    override fun onSsePoll(result: Result<Any?>, duration: Duration) {
        println("SSE poll: ${result.isSuccess}, duration=$duration")
    }
}

val telemetry = MyTelemetry()
val clientWithTelemetry = chatClient.withTelemetry(
    request = telemetry,
    sse = telemetry,
)
```

## Building Custom Prompts

```kotlin
import ai.solace.coder.api.common.*

val prompt = Prompt(
    instructions = "You are a code review assistant.",
    input = listOf(/* ResponseItems */),
    tools = listOf(/* Tool definitions as JsonElement */),
    parallelToolCalls = true,
    outputSchema = null, // Optional JSON schema
)

// Create text controls for responses API
val textControls = createTextParamForRequest(
    verbosity = VerbosityConfig.Medium,
    outputSchema = null,
)
```

## What's Working Now

✅ Provider configuration and URL building  
✅ Request builders (ChatRequest, ResponsesRequest)  
✅ Header management and auth injection  
✅ Rate limit header parsing  
✅ Error type definitions  
✅ Telemetry interfaces  

## What Needs Implementation

🚧 SSE stream parsing (`spawnChatStream`, `spawnResponsesStream`)  
🚧 Retry policy with exponential backoff  
🚧 Complete message transformation logic in ChatRequestBuilder  
🚧 ResponseStream channel/Flow implementation  
🚧 CompactClient POST implementation  
🚧 codex-protocol type integration (ResponseItem, TokenUsage, etc.)  

## Testing

Once SSE parsing is implemented, you can test with:

```kotlin
import kotlinx.coroutines.runBlocking

runBlocking {
    val stream = chatClient.streamRequest(request).getOrThrow()
    // Process events
    // stream.next() will return Result<ResponseEvent?>
}
```

See `docs/codex-api-port-status.md` for detailed implementation status and TODOs.

