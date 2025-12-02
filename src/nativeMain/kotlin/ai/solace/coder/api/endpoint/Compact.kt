// port-lint: source codex-rs/codex-api/src/endpoint/compact.rs
package ai.solace.coder.api.endpoint

import ai.solace.coder.api.AuthProvider
import ai.solace.coder.api.addAuthHeaders
import ai.solace.coder.api.common.CompactionInput
import ai.solace.coder.api.error.ApiError
import ai.solace.coder.api.provider.Provider
import ai.solace.coder.api.provider.WireApi
import ai.solace.coder.api.telemetry.RequestTelemetry
import ai.solace.coder.protocol.ResponseItem
import io.ktor.client.*
import io.ktor.client.request.*
import io.ktor.client.statement.*
import io.ktor.http.*
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.*

/** Client for the compaction endpoint. */
class CompactClient<A : AuthProvider>(
    private val httpClient: HttpClient,
    private val provider: Provider,
    private val auth: A,
) {
    private var requestTelemetry: RequestTelemetry? = null

    fun withTelemetry(request: RequestTelemetry?): CompactClient<A> {
        requestTelemetry = request
        return this
    }

    private fun path(): Result<String> {
        return when (provider.wire) {
            WireApi.Compact, WireApi.Responses -> Result.success("responses/compact")
            WireApi.Chat -> Result.failure(
                Exception("compact endpoint requires responses wire api")
            )
        }
    }

    /**
     * Compact the given JSON body.
     * Lower-level method that accepts pre-serialized JSON.
     */
    suspend fun compact(
        body: JsonElement,
        configureExtraHeaders: HttpRequestBuilder.() -> Unit = {},
    ): Result<List<ResponseItem>> {
        val pathResult = path()
        val url = provider.urlForPath(pathResult.getOrElse { return Result.failure(it) })

        return try {
            val response: HttpResponse = httpClient.post(url) {
                contentType(ContentType.Application.Json)
                setBody(body.toString())

                // Add provider default headers
                provider.defaultHeaders.forEach { (key, value) ->
                    headers.append(key, value)
                }

                // Apply extra headers
                configureExtraHeaders()

                // Add auth headers
                addAuthHeaders(auth)

                // TODO: Apply retry policy and telemetry
            }

            if (response.status.isSuccess()) {
                val responseText = response.bodyAsText()
                // Parse CompactHistoryResponse
                val json = Json.parseToJsonElement(responseText).jsonObject
                val output = json["output"]?.jsonArray ?: return Result.failure(
                    Exception("Missing 'output' field in compact response")
                )

                // TODO: Properly deserialize ResponseItem list using kotlinx.serialization
                // For now, return empty list as placeholder
                Result.success(emptyList())
            } else {
                Result.failure(Exception("HTTP ${response.status.value}: ${response.bodyAsText()}"))
            }
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    /**
     * Compact the given input.
     * Higher-level method that accepts CompactionInput and serializes it.
     */
    suspend fun compactInput(
        input: CompactionInput,
        configureExtraHeaders: HttpRequestBuilder.() -> Unit = {},
    ): Result<List<ResponseItem>> {
        return try {
            // Build JSON payload from CompactionInput
            val payload = buildJsonObject {
                put("model", input.model)
                put("instructions", input.instructions)
                put("input", JsonArray(input.input.map { item ->
                    // TODO: Serialize ResponseItem properly using kotlinx.serialization
                    buildJsonObject {
                        when (item) {
                            is ResponseItem.Message -> {
                                put("type", "message")
                                put("role", item.role)
                                put("content", JsonArray(emptyList())) // TODO: serialize content
                            }
                            else -> put("type", "other")
                        }
                    }
                }))
            }

            compact(payload, configureExtraHeaders)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }
}

/**
 * Response from compact endpoint.
 * Matches Rust CompactHistoryResponse.
 */
@Serializable
private data class CompactHistoryResponse(
    val output: List<ResponseItem>
)

