// port-lint: source codex-rs/codex-api/src/endpoint/streaming.rs
package ai.solace.coder.api.endpoint

import ai.solace.coder.api.AuthProvider
import ai.solace.coder.api.addAuthHeaders
import ai.solace.coder.api.common.ResponseStream
import ai.solace.coder.api.provider.Provider
import ai.solace.coder.api.telemetry.RequestTelemetry
import ai.solace.coder.api.telemetry.SseTelemetry
import io.ktor.client.*
import io.ktor.client.request.*
import io.ktor.http.*
import kotlinx.serialization.json.JsonElement

/**
 * Internal streaming client that handles HTTP streaming with auth and telemetry.
 * TODO: Implement full retry policy and SSE spawning logic.
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

    suspend fun stream(
        path: String,
        body: JsonElement,
        configureExtraHeaders: HttpRequestBuilder.() -> Unit,
        spawner: suspend (HttpClient, String, SseTelemetry?) -> ResponseStream,
    ): Result<ResponseStream> {
        return try {
            // Build request
            val requestBuilder = provider.buildRequest(HttpMethod.Post, path) {
                configureExtraHeaders()
                headers.append(HttpHeaders.Accept, "text/event-stream")
                setBody(body.toString())
                addAuthHeaders(auth)
            }

            // TODO: Apply retry policy with telemetry
            // For now, spawn stream directly
            val url = requestBuilder.url.buildString()
            val stream = spawner(httpClient, url, sseTelemetry)
            Result.success(stream)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }
}

