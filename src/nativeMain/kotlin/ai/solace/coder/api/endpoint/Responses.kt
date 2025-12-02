// port-lint: source codex-rs/codex-api/src/endpoint/responses.rs
package ai.solace.coder.api.endpoint

import ai.solace.coder.api.AuthProvider
import ai.solace.coder.api.common.Prompt
import ai.solace.coder.api.common.Reasoning
import ai.solace.coder.api.common.ResponseStream
import ai.solace.coder.api.common.TextControls
import ai.solace.coder.api.provider.Provider
import ai.solace.coder.api.requests.ResponsesRequest
import ai.solace.coder.api.requests.ResponsesRequestBuilder
import ai.solace.coder.api.telemetry.RequestTelemetry
import ai.solace.coder.api.telemetry.SseTelemetry
import io.ktor.client.*

/** Options for configuring ResponsesClient. */
data class ResponsesOptions(
    val reasoning: Reasoning? = null,
    val include: List<String> = emptyList(),
    val promptCacheKey: String? = null,
    val text: TextControls? = null,
    val storeOverride: Boolean? = null,
    val conversationId: String? = null,
    val sessionSource: ai.solace.coder.protocol.SessionSource? = null,
)

/** Client for Responses endpoint. */
class ResponsesClient<A : AuthProvider>(
    httpClient: HttpClient,
    provider: Provider,
    auth: A,
) {
    private val streaming: StreamingClient<A> = StreamingClient(httpClient, provider, auth)

    fun withTelemetry(
        request: RequestTelemetry?,
        sse: SseTelemetry?,
    ): ResponsesClient<A> {
        streaming.withTelemetry(request, sse)
        return this
    }

    suspend fun streamRequest(request: ResponsesRequest): Result<ResponseStream> {
        return stream(request.body, request.configureHeaders)
    }

    suspend fun streamPrompt(
        model: String,
        prompt: Prompt,
        options: ResponsesOptions = ResponsesOptions(),
    ): Result<ResponseStream> {
        val request = ResponsesRequestBuilder(model, prompt.instructions, prompt.input)
            .tools(prompt.tools)
            .parallelToolCalls(prompt.parallelToolCalls)
            .reasoning(options.reasoning)
            .include(options.include)
            .promptCacheKey(options.promptCacheKey)
            .text(options.text)
            .storeOverride(options.storeOverride)
            .conversation(options.conversationId)
            .sessionSource(options.sessionSource)
            .build(streaming.provider())
            .getOrElse { return Result.failure(it) }
        return streamRequest(request)
    }

    private suspend fun stream(
        body: kotlinx.serialization.json.JsonElement,
        configureExtraHeaders: io.ktor.client.request.HttpRequestBuilder.() -> Unit,
    ): Result<ResponseStream> {
        // TODO: Implement spawnResponsesStream once SSE parsing is ported
        return streaming.stream("responses", body, configureExtraHeaders) { _, _, _ ->
            TODO("spawnResponsesStream not yet implemented")
        }
    }
}

