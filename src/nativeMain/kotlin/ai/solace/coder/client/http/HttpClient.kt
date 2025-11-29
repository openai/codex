package ai.solace.coder.client.http

import ai.solace.coder.client.auth.AuthManager
import ai.solace.coder.client.streaming.SseParser
import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.protocol.models.ResponseEvent
import ai.solace.coder.protocol.models.ResponseItem
import io.ktor.client.*
import io.ktor.client.engine.curl.*
import io.ktor.client.plugins.contentnegotiation.*

import io.ktor.client.request.*
import io.ktor.client.statement.*
import io.ktor.http.*
import io.ktor.serialization.kotlinx.json.*
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.serialization.json.Json

/**
 * HTTP client for making requests to the Codex backend API.
 * Handles POST to /responses endpoint and streaming SSE responses.
 *
 * Maps to Rust's backend-client and ModelClient functionality.
 *
 * TODO: Port from Rust codex-rs/core/src/client.rs and codex-rs/backend-client/:
 * - [ ] ModelClient trait with full streaming response handling
 * - [ ] Prompt construction with developer/base/user instructions
 * - [ ] Tool specification serialization for API
 * - [ ] Model provider detection (Anthropic, OpenAI, local OSS)
 * - [ ] WireApi enum (Anthropic, OpenAI, Google, Bedrock)
 * - [ ] Rate limit parsing from response headers
 * - [ ] Context window detection per model
 * - [ ] Exponential backoff with jitter for retries
 * - [ ] OpenTelemetry integration for tracing
 * - [ ] Response event parsing (AgentMessage, ToolCall, Reasoning, etc.)
 * - [ ] ModelProviderInfo for provider-specific configuration
 */
class CodexHttpClient(
    private val baseUrl: String,
    private val authManager: AuthManager,
    private val maxRetries: Int = 3
) {
    private val httpClient = HttpClient(Curl) {
        install(ContentNegotiation) {
            json(Json {
                ignoreUnknownKeys = true
                prettyPrint = false
                isLenient = true
            })
        }
        
        engine {
            // Configure timeout
        }
    }
    
    /**
     * Stream a prompt to the /responses endpoint and receive SSE events.
     */
    fun streamPrompt(
        model: String,
        prompt: ResponsesPrompt,
        options: ResponsesOptions = ResponsesOptions()
    ): Flow<CodexResult<ResponseEvent>> = flow {
        var retries = 0
        var lastError: CodexError? = null
        
        while (retries < maxRetries) {
            try {
                val response = makeStreamingRequest(model, prompt, options)
                
                if (response.status.isSuccess()) {
                    // Parse SSE stream
                    val parser = SseParser()
                    val bodyText = response.bodyAsText()
                    
                    for (event in parser.parse(bodyText)) {
                        emit(CodexResult.success(event))
                    }
                    
                    // Successfully completed
                    return@flow
                } else {
                    lastError = CodexError.Http(
                        statusCode = response.status.value,
                        message = response.bodyAsText()
                    )
                    
                    // Check if we should retry
                    if (shouldRetry(response.status.value)) {
                        retries++
                        if (retries < maxRetries) {
                            // Exponential backoff
                            kotlinx.coroutines.delay(calculateBackoff(retries))
                            continue
                        }
                    }
                    
                    // Non-retryable error or max retries reached
                    emit(CodexResult.failure(lastError))
                    return@flow
                }
            } catch (e: Exception) {
                lastError = CodexError.Io(e.message ?: "Unknown error")
                retries++
                
                if (retries < maxRetries) {
                    kotlinx.coroutines.delay(calculateBackoff(retries))
                    continue
                }
                
                emit(CodexResult.failure(lastError))
                return@flow
            }
        }
        
        // Max retries exceeded
        emit(CodexResult.failure(
            lastError ?: CodexError.Io("Max retries exceeded")
        ))
    }
    
    /**
     * Make a streaming request to the API.
     */
    private suspend fun makeStreamingRequest(
        model: String,
        prompt: ResponsesPrompt,
        options: ResponsesOptions
    ): HttpResponse {
        val authHeader = authManager.getAuthorizationHeader()
        
        return httpClient.post("$baseUrl/responses") {
            header(HttpHeaders.ContentType, ContentType.Application.Json)
            
            if (authHeader != null) {
                header(HttpHeaders.Authorization, authHeader)
            }
            
            // Add custom headers
            header("x-model", model)
            options.conversationId?.let { header("x-conversation-id", it) }
            options.sessionSource?.let { header("x-session-source", it) }
            
            setBody(prompt)
        }
    }
    
    /**
     * Determine if an HTTP status code should trigger a retry.
     */
    private fun shouldRetry(statusCode: Int): Boolean {
        return statusCode in setOf(429, 500, 502, 503, 504)
    }
    
    /**
     * Calculate exponential backoff delay for retries.
     */
    private fun calculateBackoff(retryCount: Int): Long {
        val baseDelay = 1000L // 1 second
        val maxDelay = 16000L // 16 seconds
        val delay = baseDelay * (1L shl (retryCount - 1))
        return minOf(delay, maxDelay)
    }
    
    /**
     * Close the HTTP client.
     */
    fun close() {
        httpClient.close()
    }
}

/**
 * Prompt payload for the /responses endpoint.
 */
data class ResponsesPrompt(
    val instructions: String,
    val input: List<ResponseItem>,
    val tools: List<Any> = emptyList(),
    val parallelToolCalls: Boolean = false,
    val outputSchema: kotlinx.serialization.json.JsonElement? = null
)

/**
 * Options for /responses requests.
 */
data class ResponsesOptions(
    val reasoning: ReasoningConfig? = null,
    val include: List<String> = emptyList(),
    val promptCacheKey: String? = null,
    val text: TextOptions? = null,
    val conversationId: String? = null,
    val sessionSource: String? = null
)

/**
 * Reasoning configuration for the API.
 * Renamed from Reasoning to avoid conflict with ResponseItem.Reasoning
 */
data class ReasoningConfig(
    val effort: String? = null,
    val summary: Boolean = true
)

/**
 * Text output options.
 */
data class TextOptions(
    val verbosity: String? = null
)

// ResponseEvent is now imported from ai.solace.coder.protocol.models