// port-lint: source codex-rs/core/src/client.rs
package ai.solace.coder.core.client

import ai.solace.coder.api.AuthProvider
import ai.solace.coder.api.endpoint.ChatClient
import ai.solace.coder.api.endpoint.CompactClient
import ai.solace.coder.api.endpoint.ResponsesClient
import ai.solace.coder.api.endpoint.ResponsesOptions
import ai.solace.coder.api.endpoint.aggregate
import ai.solace.coder.api.endpoint.streamingMode
import ai.solace.coder.api.common.CompactionInput
import ai.solace.coder.api.common.Prompt as ApiPrompt
import ai.solace.coder.api.common.Reasoning
import ai.solace.coder.api.common.ResponseEvent
import ai.solace.coder.api.common.ResponseStream as ApiResponseStream
import ai.solace.coder.api.common.createTextParamForRequest
import ai.solace.coder.api.error.ApiError
import ai.solace.coder.api.provider.WireApi
import ai.solace.coder.api.telemetry.RequestTelemetry
import ai.solace.coder.api.telemetry.SseTelemetry
import ai.solace.coder.core.AuthManager
import ai.solace.coder.core.AuthMode
import ai.solace.coder.core.CodexAuth
import ai.solace.coder.core.config.Config
import ai.solace.coder.core.error.CodexErr
import ai.solace.coder.core.model.ModelFamily
import ai.solace.coder.core.model.ModelProviderInfo
import ai.solace.coder.core.prompt.Prompt
import ai.solace.coder.protocol.ConversationId
import ai.solace.coder.protocol.ReasoningEffortConfig
import ai.solace.coder.protocol.ReasoningSummaryConfig
import ai.solace.coder.protocol.ResponseItem
import ai.solace.coder.protocol.SessionSource
import io.ktor.client.*
import io.ktor.http.*
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.serialization.json.JsonElement
import kotlin.time.Duration

/**
 * Main client for streaming model interactions.
 *
 * Manages authentication, API client creation, and streaming responses
 * through the Chat Completions or Responses API endpoints.
 *
 * Mirrors Rust's ModelClient from core/src/client.rs
 */
class ModelClient(
    private val config: Config,
    private val authManager: AuthManager?,
    private val otelEventManager: OtelEventManager,
    private val provider: ModelProviderInfo,
    private val conversationId: ConversationId,
    private val effort: ReasoningEffortConfig?,
    private val summary: ReasoningSummaryConfig,
    private val sessionSource: SessionSource,
) {

    /**
     * Get the effective model context window accounting for the configured percentage.
     */
    fun getModelContextWindow(): Long? {
        val pct = config.modelFamily.effectiveContextWindowPercent
        val window = config.modelContextWindow
            ?: getModelInfo(config.modelFamily)?.contextWindow
            ?: return null
        return (window * pct) / 100
    }

    /**
     * Get the auto-compact token limit for this model.
     */
    fun getAutoCompactTokenLimit(): Long? {
        return config.modelAutoCompactTokenLimit
            ?: getModelInfo(config.modelFamily)?.autoCompactTokenLimit
    }

    fun config(): Config = config

    fun provider(): ModelProviderInfo = provider

    /**
     * Streams a single model turn using either the Responses or Chat
     * Completions wire API, depending on the configured provider.
     *
     * For Chat providers, the underlying stream is optionally aggregated
     * based on the `showRawAgentReasoning` flag in the config.
     */
    suspend fun stream(prompt: Prompt): Result<ResponseStream> {
        return when (provider.wireApi) {
            WireApi.Responses -> streamResponsesApi(prompt)
            WireApi.Chat -> {
                val apiStream = streamChatCompletions(prompt).getOrElse {
                    return Result.failure(it)
                }

                val processedStream = if (config.showRawAgentReasoning) {
                    apiStream.streamingMode()
                } else {
                    apiStream.aggregate()
                }

                Result.success(mapResponseStream(processedStream, otelEventManager))
            }
            else -> Result.failure(Exception("Unsupported wire API: ${provider.wireApi}"))
        }
    }

    /**
     * Streams a turn via the OpenAI Chat Completions API.
     *
     * This path is only used when the provider is configured with
     * `WireApi.Chat`; it does not support `outputSchema` today.
     */
    private suspend fun streamChatCompletions(prompt: Prompt): Result<ApiResponseStream> {
        if (prompt.outputSchema != null) {
            return Result.failure(
                CodexErr.UnsupportedOperation(
                    "output_schema is not supported for Chat Completions API"
                )
            )
        }

        val instructions = prompt.getFullInstructions(config.modelFamily)
        val toolsJson = createToolsJsonForChatCompletionsApi(prompt.tools)
            ?: return Result.failure(Exception("Failed to create tools JSON"))
        val apiPrompt = buildApiPrompt(prompt, instructions, toolsJson)
        val conversationIdStr = conversationId.toString()
        val sessionSourceClone = sessionSource

        var refreshed = false
        while (true) {
            val auth = authManager?.auth()
            val apiProvider = provider.toApiProvider(auth?.mode)
                ?: return Result.failure(Exception("Failed to create API provider"))
            val apiAuth = authProviderFromAuth(auth, provider)
                ?: return Result.failure(Exception("Failed to create API auth"))

            val httpClient = buildHttpClient()
            val (requestTelemetry, sseTelemetry) = buildStreamingTelemetry()
            val client = ChatClient(httpClient, apiProvider, apiAuth)
                .withTelemetry(requestTelemetry, sseTelemetry)

            val streamResult = client.streamPrompt(
                model = config.model,
                prompt = apiPrompt,
                conversationId = conversationIdStr,
                sessionSource = sessionSourceClone
            )

            return when {
                streamResult.isSuccess -> streamResult
                isUnauthorizedError(streamResult) -> {
                    handleUnauthorized(
                        HttpStatusCode.Unauthorized,
                        refreshed,
                        authManager,
                        auth
                    ).getOrElse { return Result.failure(it) }
                    refreshed = true
                    continue
                }
                else -> Result.failure(
                    streamResult.exceptionOrNull() ?: Exception("Unknown error")
                )
            }
        }
    }

    /**
     * Streams a turn via the OpenAI Responses API.
     *
     * Handles SSE fixtures, reasoning summaries, verbosity, and the
     * `text` controls used for output schemas.
     */
    private suspend fun streamResponsesApi(prompt: Prompt): Result<ResponseStream> {
        // TODO: Handle SSE fixture loading if CODEX_RS_SSE_FIXTURE is set

        val instructions = prompt.getFullInstructions(config.modelFamily)
        val toolsJson = createToolsJsonForResponsesApi(prompt.tools)
            ?: return Result.failure(Exception("Failed to create tools JSON"))

        val reasoning = if (config.modelFamily.supportsReasoningSummaries) {
            Reasoning(
                effort = effort ?: config.modelFamily.defaultReasoningEffort,
                summary = summary
            )
        } else {
            null
        }

        val include = if (reasoning != null) {
            listOf("reasoning.encrypted_content")
        } else {
            emptyList()
        }

        val verbosity = if (config.modelFamily.supportVerbosity) {
            config.modelVerbosity ?: config.modelFamily.defaultVerbosity
        } else {
            if (config.modelVerbosity != null) {
                // TODO: Log warning about verbosity being ignored
            }
            null
        }

        val text = createTextParamForRequest(verbosity, prompt.outputSchema)
        val apiPrompt = buildApiPrompt(prompt, instructions, toolsJson)
        val conversationIdStr = conversationId.toString()
        val sessionSourceClone = sessionSource

        var refreshed = false
        while (true) {
            val auth = authManager?.auth()
            val apiProvider = provider.toApiProvider(auth?.mode)
                ?: return Result.failure(Exception("Failed to create API provider"))
            val apiAuth = authProviderFromAuth(auth, provider)
                ?: return Result.failure(Exception("Failed to create API auth"))

            val httpClient = buildHttpClient()
            val (requestTelemetry, sseTelemetry) = buildStreamingTelemetry()
            val client = ResponsesClient(httpClient, apiProvider, apiAuth)
                .withTelemetry(requestTelemetry, sseTelemetry)

            val options = ResponsesOptions(
                reasoning = reasoning,
                include = include,
                promptCacheKey = conversationIdStr,
                text = text,
                storeOverride = null,
                conversationId = conversationIdStr,
                sessionSource = sessionSourceClone
            )

            val streamResult = client.streamPrompt(
                model = config.model,
                prompt = apiPrompt,
                options = options
            )

            return when {
                streamResult.isSuccess -> {
                    val stream = streamResult.getOrThrow()
                    Result.success(mapResponseStream(stream, otelEventManager))
                }
                isUnauthorizedError(streamResult) -> {
                    handleUnauthorized(
                        HttpStatusCode.Unauthorized,
                        refreshed,
                        authManager,
                        auth
                    ).getOrElse { return Result.failure(it) }
                    refreshed = true
                    continue
                }
                else -> Result.failure(
                    streamResult.exceptionOrNull() ?: Exception("Unknown error")
                )
            }
        }
    }

    fun getProvider(): ModelProviderInfo = provider

    fun getOtelEventManager(): OtelEventManager = otelEventManager

    fun getSessionSource(): SessionSource = sessionSource

    fun getModel(): String = config.model

    fun getModelFamily(): ModelFamily = config.modelFamily

    fun getReasoningEffort(): ReasoningEffortConfig? = effort

    fun getReasoningSummary(): ReasoningSummaryConfig = summary

    fun getAuthManager(): AuthManager? = authManager

    /**
     * Compacts the current conversation history using the Compact endpoint.
     *
     * This is a unary call (no streaming) that returns a new list of
     * `ResponseItem`s representing the compacted transcript.
     */
    suspend fun compactConversationHistory(prompt: Prompt): Result<List<ResponseItem>> {
        if (prompt.input.isEmpty()) {
            return Result.success(emptyList())
        }

        val auth = authManager?.auth()
        val apiProvider = provider.toApiProvider(auth?.mode)
            ?: return Result.failure(Exception("Failed to create API provider"))
        val apiAuth = authProviderFromAuth(auth, provider)
            ?: return Result.failure(Exception("Failed to create API auth"))

        val httpClient = buildHttpClient()
        val requestTelemetry = buildRequestTelemetry()
        val client = CompactClient(httpClient, apiProvider, apiAuth)
            .withTelemetry(requestTelemetry)

        val instructions = prompt.getFullInstructions(config.modelFamily)
        val payload = CompactionInput(
            model = config.model,
            input = prompt.input,
            instructions = instructions
        )

        // Build extra headers for subagent
        val configureHeaders: io.ktor.client.request.HttpRequestBuilder.() -> Unit = {
            if (sessionSource is SessionSource.SubAgent) {
                // TODO: Extract SubAgentSource value once SessionSource is a sealed class
                val subagent = "review" // Placeholder
                headers.append("x-openai-subagent", subagent)
            }
        }

        return client.compactInput(payload, configureHeaders)
    }

    /**
     * Builds request and SSE telemetry for streaming API calls (Chat/Responses).
     */
    private fun buildStreamingTelemetry(): Pair<RequestTelemetry, SseTelemetry> {
        val telemetry = ApiTelemetry(otelEventManager)
        return Pair(telemetry, telemetry)
    }

    /**
     * Builds request telemetry for unary API calls (e.g., Compact endpoint).
     */
    private fun buildRequestTelemetry(): RequestTelemetry {
        return ApiTelemetry(otelEventManager)
    }
}

/**
 * Adapts the core `Prompt` type into the `codex-api` payload shape.
 */
private fun buildApiPrompt(prompt: Prompt, instructions: String, toolsJson: List<JsonElement>): ApiPrompt {
    return ApiPrompt(
        instructions = instructions,
        input = prompt.getFormattedInput(),
        tools = toolsJson,
        parallelToolCalls = prompt.parallelToolCalls,
        outputSchema = prompt.outputSchema
    )
}

/**
 * Maps an API response stream to the core ResponseStream type.
 */
private fun mapResponseStream(
    apiStream: ApiResponseStream,
    otelEventManager: OtelEventManager
): ResponseStream {
    // TODO: Implement full stream mapping with telemetry events
    // For now, return a placeholder
    return ResponseStream(
        flow {
            // Poll the API stream and emit events
            var result = apiStream.next()
            while (result.isSuccess && result.getOrNull() != null) {
                val event = result.getOrNull()!!

                // Handle completion events for telemetry
                if (event is ResponseEvent.Completed) {
                    event.tokenUsage?.let { usage ->
                        otelEventManager.sseEventCompleted(
                            inputTokens = usage.inputTokens,
                            outputTokens = usage.outputTokens,
                            cachedInputTokens = usage.cachedInputTokens,
                            reasoningOutputTokens = usage.reasoningOutputTokens,
                            totalTokens = usage.totalTokens
                        )
                    }
                }

                emit(Result.success(event))
                result = apiStream.next()
            }

            // Handle errors
            if (result.isFailure) {
                val error = result.exceptionOrNull() ?: Exception("Unknown error")
                otelEventManager.seeEventCompletedFailed(error)
                emit(Result.failure(error))
            }
        }
    )
}

/**
 * Handles a 401 response by optionally refreshing ChatGPT tokens once.
 */
private suspend fun handleUnauthorized(
    status: HttpStatusCode,
    refreshed: Boolean,
    authManager: AuthManager?,
    auth: CodexAuth?
): Result<Unit> {
    if (refreshed) {
        return Result.failure(mapUnauthorizedStatus(status))
    }

    if (authManager != null && auth != null && auth.mode == AuthMode.ChatGPT) {
        return when (val refreshResult = authManager.refreshToken()) {
            is Result.Success -> Result.success(Unit)
            is Result.Failure -> Result.failure(
                CodexErr.RefreshTokenFailed(refreshResult.error.message ?: "Unknown error")
            )
        }
    }

    return Result.failure(mapUnauthorizedStatus(status))
}

private fun mapUnauthorizedStatus(status: HttpStatusCode): Exception {
    return ApiError.Transport("HTTP ${status.value}: Unauthorized")
}

private fun isUnauthorizedError(result: Result<*>): Boolean {
    val error = result.exceptionOrNull()
    return error is ApiError.Transport && error.message?.contains("401") == true
}

/**
 * Telemetry implementation for API requests and SSE events.
 */
private class ApiTelemetry(
    private val otelEventManager: OtelEventManager
) : RequestTelemetry, SseTelemetry {

    override fun onRequest(
        attempt: Long,
        status: HttpStatusCode?,
        error: Exception?,
        duration: Duration
    ) {
        otelEventManager.recordApiRequest(
            attempt = attempt,
            status = status?.value,
            errorMessage = error?.message,
            duration = duration
        )
    }

    override fun onSsePoll(
        result: Result<*>,
        duration: Duration
    ) {
        otelEventManager.logSseEvent(result, duration)
    }
}

/**
 * Core ResponseStream wrapper.
 * TODO: This should be defined in core, not here.
 */
data class ResponseStream(
    val events: Flow<Result<ResponseEvent>>
)

/**
 * Placeholder for OtelEventManager.
 * TODO: Port from codex-otel crate.
 */
class OtelEventManager {
    fun sseEventCompleted(
        inputTokens: Long,
        outputTokens: Long,
        cachedInputTokens: Long?,
        reasoningOutputTokens: Long?,
        totalTokens: Long
    ) {
        // TODO: Implement telemetry logging
    }

    fun seeEventCompletedFailed(error: Throwable) {
        // TODO: Implement error telemetry
    }

    fun recordApiRequest(
        attempt: Long,
        status: Int?,
        errorMessage: String?,
        duration: Duration
    ) {
        // TODO: Implement request telemetry
    }

    fun logSseEvent(result: Result<*>, duration: Duration) {
        // TODO: Implement SSE event telemetry
    }
}




// Placeholder functions that need to be ported from other modules

private fun getModelInfo(modelFamily: ModelFamily): ModelInfo? {
    // TODO: Port from openai_model_info.rs
    return null
}

private fun createToolsJsonForChatCompletionsApi(tools: List<Any>): List<JsonElement>? {
    // TODO: Port from tools/spec.rs
    return emptyList()
}

private fun createToolsJsonForResponsesApi(tools: List<Any>): List<JsonElement>? {
    // TODO: Port from tools/spec.rs
    return emptyList()
}

private fun authProviderFromAuth(auth: CodexAuth?, provider: ModelProviderInfo): AuthProvider? {
    // TODO: Port from api_bridge.rs
    return null
}

private fun buildHttpClient(): HttpClient {
    // TODO: Port from default_client.rs
    return HttpClient()
}

data class ModelInfo(
    val contextWindow: Long,
    val autoCompactTokenLimit: Long?
)

