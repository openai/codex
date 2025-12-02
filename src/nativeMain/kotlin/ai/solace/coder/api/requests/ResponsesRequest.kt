// port-lint: source codex-rs/codex-api/src/requests/responses.rs
package ai.solace.coder.api.requests

import ai.solace.coder.api.common.Reasoning
import ai.solace.coder.api.common.ResponsesApiRequest
import ai.solace.coder.api.common.TextControls
import ai.solace.coder.api.error.ApiError
import ai.solace.coder.api.provider.Provider
import io.ktor.client.request.*
import kotlinx.serialization.json.*
import kotlinx.serialization.encodeToString

/** Assembled request body plus header configuration for a Responses stream request. */
data class ResponsesRequest(
    val body: JsonElement,
    val configureHeaders: HttpRequestBuilder.() -> Unit,
)

/** Builder for ResponsesRequest. */
class ResponsesRequestBuilder(
    private var model: String? = null,
    private var instructions: String? = null,
    private var input: List<ai.solace.coder.protocol.ResponseItem>? = null,
) {
    private var tools: List<JsonElement>? = null
    private var parallelToolCalls: Boolean = false
    private var reasoning: Reasoning? = null
    private var include: List<String> = emptyList()
    private var promptCacheKey: String? = null
    private var text: TextControls? = null
    private var conversationId: String? = null
    private var sessionSource: ai.solace.coder.protocol.SessionSource? = null
    private var storeOverride: Boolean? = null
    private val extraHeaders: MutableMap<String, String> = mutableMapOf()

    constructor(model: String, instructions: String, input: List<ai.solace.coder.protocol.ResponseItem>) : this() {
        this.model = model
        this.instructions = instructions
        this.input = input
    }

    fun tools(tools: List<JsonElement>): ResponsesRequestBuilder {
        this.tools = tools
        return this
    }

    fun parallelToolCalls(enabled: Boolean): ResponsesRequestBuilder {
        this.parallelToolCalls = enabled
        return this
    }

    fun reasoning(reasoning: Reasoning?): ResponsesRequestBuilder {
        this.reasoning = reasoning
        return this
    }

    fun include(include: List<String>): ResponsesRequestBuilder {
        this.include = include
        return this
    }

    fun promptCacheKey(key: String?): ResponsesRequestBuilder {
        this.promptCacheKey = key
        return this
    }

    fun text(text: TextControls?): ResponsesRequestBuilder {
        this.text = text
        return this
    }

    fun conversation(conversationId: String?): ResponsesRequestBuilder {
        this.conversationId = conversationId
        return this
    }

    fun sessionSource(source: ai.solace.coder.protocol.SessionSource?): ResponsesRequestBuilder {
        sessionSource = source
        return this
    }

    fun storeOverride(store: Boolean?): ResponsesRequestBuilder {
        this.storeOverride = store
        return this
    }

    fun extraHeaders(headers: Map<String, String>): ResponsesRequestBuilder {
        this.extraHeaders.putAll(headers)
        return this
    }

    fun build(provider: Provider): Result<ResponsesRequest> {
        return try {
            val modelVal = model ?: return Result.failure(
                IllegalArgumentException("missing model for responses request")
            )
            val instructionsVal = instructions ?: return Result.failure(
                IllegalArgumentException("missing instructions for responses request")
            )
            val inputVal = input ?: return Result.failure(
                IllegalArgumentException("missing input for responses request")
            )
            val toolsVal = tools ?: emptyList()

            val store = storeOverride ?: provider.isAzureResponsesEndpoint()

            // TODO: Build proper ResponsesApiRequest using kotlinx.serialization
            // For now, build JSON manually
            val bodyJson = buildJsonObject {
                put("model", modelVal)
                put("instructions", instructionsVal)
                put("input", JsonArray(inputVal.map { item ->
                    // TODO: Use proper kotlinx.serialization once ResponseItem has @Serializable
                    buildJsonObject {
                        when (item) {
                            is ai.solace.coder.protocol.ResponseItem.Message -> {
                                put("type", "message")
                                put("role", item.role)
                                put("content", JsonArray(emptyList())) // TODO: serialize content items
                                item.id?.let { put("id", it) }
                            }
                            else -> {
                                // TODO: Handle other ResponseItem types
                                put("type", "other")
                            }
                        }
                    }
                }))
                put("tools", JsonArray(toolsVal))
                put("tool_choice", "auto")
                put("parallel_tool_calls", parallelToolCalls)
                reasoning?.let { put("reasoning", it.toJson()) }
                put("store", store)
                put("stream", true)
                put("include", JsonArray(include.map { JsonPrimitive(it) }))
                promptCacheKey?.let { put("prompt_cache_key", it) }
                text?.let { put("text", it.toJson()) }
            }

            var body: JsonElement = bodyJson
            if (store && provider.isAzureResponsesEndpoint()) {
                body = attachItemIds(body, inputVal)
            }

            val configureHeaders: HttpRequestBuilder.() -> Unit = {
                extraHeaders.forEach { (key, value) ->
                    headers.append(key, value)
                }
                buildConversationHeaders(conversationId, this)
                subagentHeader(sessionSource)?.let { subagent ->
                    insertHeader(this, "x-openai-subagent", subagent)
                }
            }

            Result.success(ResponsesRequest(body, configureHeaders))
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    private fun attachItemIds(payloadJson: JsonElement, originalItems: List<ai.solace.coder.protocol.ResponseItem>): JsonElement {
        // TODO: Implement ID attachment logic for Azure endpoints
        // This should iterate through the JSON array and add 'id' fields from originalItems
        return payloadJson
    }
}


// Extension helpers for serialization (temporary until proper types exist)
private fun Reasoning.toJson(): JsonElement = buildJsonObject {
    effort?.let { put("effort", JsonPrimitive(it.toString())) }
    summary?.let { put("summary", JsonPrimitive(it.toString())) }
}

private fun TextControls.toJson(): JsonElement = buildJsonObject {
    verbosity?.let { put("verbosity", JsonPrimitive(it.name.lowercase())) }
    format?.let { fmt ->
        put("format", buildJsonObject {
            put("type", JsonPrimitive("json_schema"))
            put("strict", fmt.strict)
            put("schema", JsonPrimitive(fmt.schema.toString()))
            put("name", fmt.name)
        })
    }
}

