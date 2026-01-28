// port-lint: source codex-rs/codex-api/src/endpoint/streaming.rs
package ai.solace.coder.api.endpoint

import ai.solace.coder.api.AuthProvider
import ai.solace.coder.api.addAuthHeaders
import ai.solace.coder.api.common.ResponseStream
import ai.solace.coder.api.error.ApiError
import ai.solace.coder.api.provider.Provider
import ai.solace.coder.api.ratelimits.parseRateLimit
import ai.solace.coder.api.sse.ChannelResponseStream
import ai.solace.coder.api.sse.processChatSse
import ai.solace.coder.api.sse.processSse
import ai.solace.coder.api.telemetry.RequestTelemetry
import ai.solace.coder.api.telemetry.SseTelemetry
import ai.solace.coder.protocol.ResponseEvent
import io.ktor.client.*
import io.ktor.client.request.*
import io.ktor.client.statement.*
import io.ktor.http.*
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement

/**
 * SSE stream type determines which parser to use.
 */
enum class SseStreamType {
    /** Chat Completions API format. */
    Chat,
    /** Responses API format. */
    Responses
}

/**
 * Internal streaming client that handles HTTP streaming with auth and telemetry.
 *
 * Ported from codex-rs/codex-api/src/endpoint/streaming.rs
 */
internal class StreamingClient<A : AuthProvider>(
    private val httpClient: HttpClient,
    private val provider: Provider,
    private val auth: A,
) {
    private var requestTelemetry: RequestTelemetry? = null
    private var sseTelemetry: SseTelemetry? = null

    fun withTelemetry(
        request: RequestTelemetry?,
        sse: SseTelemetry?,
    ): StreamingClient<A> {
        requestTelemetry = request
        sseTelemetry = sse
        return this
    }

    fun provider(): Provider = provider

    /**
     * Send a streaming HTTP request and parse the SSE response.
     */
    suspend fun stream(
        path: String,
        body: JsonElement,
        configureExtraHeaders: HttpRequestBuilder.() -> Unit,
        streamType: SseStreamType,
        scope: CoroutineScope = CoroutineScope(Dispatchers.Default)
    ): Result<ChannelResponseStream> {
        val channel = Channel<Result<ResponseEvent>>(1600)
        val idleTimeout = provider.streamIdleTimeout
        val telemetry = sseTelemetry

        scope.launch {
            try {
                val response = httpClient.post {
                    url(provider.urlForPath(path))
                    provider.defaultHeaders.forEach { (key, value) ->
                        headers.append(key, value)
                    }
                    configureExtraHeaders()
                    headers.append(HttpHeaders.Accept, "text/event-stream")
                    headers.append(HttpHeaders.ContentType, "application/json")
                    setBody(body.toString())
                    addAuthHeaders(auth)
                }

                // Check for HTTP error status
                if (!response.status.isSuccess()) {
                    val statusCode = response.status.value
                    val errorBody = response.bodyAsText()

                    val error = when (statusCode) {
                        401 -> ApiError.Unauthorized(errorBody)
                        429 -> ApiError.RateLimited(errorBody)
                        in 500..599 -> ApiError.ServerError(statusCode, errorBody)
                        else -> ApiError.HttpError(statusCode, errorBody)
                    }
                    channel.send(Result.failure(error))
                    return@launch
                }

                // Parse rate limits from response headers
                val rateLimits = parseRateLimit(response.headers)
                if (rateLimits != null) {
                    channel.send(Result.success(ResponseEvent.RateLimits(rateLimits)))
                }

                // Use appropriate SSE parser based on stream type
                when (streamType) {
                    SseStreamType.Chat -> processChatSse(response, channel, idleTimeout, telemetry)
                    SseStreamType.Responses -> processSse(response, channel, idleTimeout, telemetry)
                }
            } catch (e: Exception) {
                channel.send(Result.failure(ApiError.Stream(e.message ?: "HTTP request failed")))
            } finally {
                channel.close()
            }
        }

        return Result.success(ChannelResponseStream(channel))
    }
}
