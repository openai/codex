// port-lint: source codex-rs/codex-api/src/sse/mod.rs, codex-rs/codex-api/src/sse/chat.rs, codex-rs/codex-api/src/sse/responses.rs
package ai.solace.coder.api.sse

import ai.solace.coder.api.error.ApiError
import ai.solace.coder.api.ratelimits.parseRateLimit
import ai.solace.coder.api.telemetry.SseTelemetry
import ai.solace.coder.protocol.ContentItem
import ai.solace.coder.protocol.ReasoningItemContent
import ai.solace.coder.protocol.ResponseEvent
import ai.solace.coder.protocol.ResponseItem
import ai.solace.coder.protocol.TokenUsage
import io.ktor.client.*
import io.ktor.client.request.*
import io.ktor.client.statement.*
import io.ktor.http.*
import io.ktor.utils.io.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ClosedReceiveChannelException
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.*
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.TimeSource

/**
 * Response stream wrapper around a channel.
 */
class ChannelResponseStream(
    private val channel: Channel<Result<ResponseEvent>>
) {
    /**
     * Receive the next event, or null if stream ended.
     */
    suspend fun next(): Result<ResponseEvent>? {
        return try {
            channel.receive()
        } catch (e: ClosedReceiveChannelException) {
            null
        }
    }

    /**
     * Close the stream.
     */
    fun close() {
        channel.close()
    }
}

/**
 * Spawn a chat stream parser from an HTTP response.
 * This is for the Chat Completions API format.
 */
suspend fun spawnChatStream(
    httpClient: HttpClient,
    request: HttpRequestBuilder.() -> Unit,
    idleTimeout: Duration,
    telemetry: SseTelemetry?,
    scope: CoroutineScope = CoroutineScope(Dispatchers.Default)
): ChannelResponseStream {
    val channel = Channel<Result<ResponseEvent>>(1600)

    scope.launch {
        try {
            httpClient.prepareGet(request).execute { response ->
                processChatSse(response, channel, idleTimeout, telemetry)
            }
        } catch (e: Exception) {
            channel.send(Result.failure(ApiError.Stream(e.message ?: "HTTP request failed")))
        } finally {
            channel.close()
        }
    }

    return ChannelResponseStream(channel)
}

/**
 * Spawn a responses stream parser from an HTTP response.
 * This is for the Responses API format.
 */
suspend fun spawnResponsesStream(
    httpClient: HttpClient,
    request: HttpRequestBuilder.() -> Unit,
    idleTimeout: Duration,
    telemetry: SseTelemetry?,
    scope: CoroutineScope = CoroutineScope(Dispatchers.Default)
): ChannelResponseStream {
    val channel = Channel<Result<ResponseEvent>>(1600)

    scope.launch {
        try {
            httpClient.prepareGet(request).execute { response ->
                // Parse rate limits from headers
                val rateLimits = parseRateLimit(response.headers)
                if (rateLimits != null) {
                    channel.send(Result.success(ResponseEvent.RateLimits(rateLimits)))
                }

                processSse(response, channel, idleTimeout, telemetry)
            }
        } catch (e: Exception) {
            channel.send(Result.failure(ApiError.Stream(e.message ?: "HTTP request failed")))
        } finally {
            channel.close()
        }
    }

    return ChannelResponseStream(channel)
}

/**
 * Load an SSE stream from a test fixture file.
 */
fun streamFromFixture(
    path: String,
    idleTimeout: Duration,
    scope: CoroutineScope = CoroutineScope(Dispatchers.Default)
): ChannelResponseStream {
    val channel = Channel<Result<ResponseEvent>>(1600)

    scope.launch {
        try {
            // Read fixture file and process as SSE
            // Note: In Kotlin Native, file I/O needs platform-specific implementation
            // For now, we provide a stub that can be implemented per-platform
            channel.send(Result.failure(ApiError.Stream("Fixture loading not implemented for this platform")))
        } finally {
            channel.close()
        }
    }

    return ChannelResponseStream(channel)
}

// ============================================================================
// Internal SSE parsing for Responses API
// ============================================================================

@Serializable
private data class SseError(
    val type: String? = null,
    val code: String? = null,
    val message: String? = null,
    @kotlinx.serialization.SerialName("plan_type")
    val planType: String? = null,
    @kotlinx.serialization.SerialName("resets_at")
    val resetsAt: Long? = null
)

@Serializable
private data class ResponseCompleted(
    val id: String,
    val usage: ResponseCompletedUsage? = null
)

@Serializable
private data class ResponseCompletedUsage(
    @kotlinx.serialization.SerialName("input_tokens")
    val inputTokens: Long,
    @kotlinx.serialization.SerialName("input_tokens_details")
    val inputTokensDetails: InputTokensDetails? = null,
    @kotlinx.serialization.SerialName("output_tokens")
    val outputTokens: Long,
    @kotlinx.serialization.SerialName("output_tokens_details")
    val outputTokensDetails: OutputTokensDetails? = null,
    @kotlinx.serialization.SerialName("total_tokens")
    val totalTokens: Long
)

@Serializable
private data class InputTokensDetails(
    @kotlinx.serialization.SerialName("cached_tokens")
    val cachedTokens: Long = 0
)

@Serializable
private data class OutputTokensDetails(
    @kotlinx.serialization.SerialName("reasoning_tokens")
    val reasoningTokens: Long = 0
)

private fun ResponseCompletedUsage.toTokenUsage(): TokenUsage {
    return TokenUsage(
        inputTokens = inputTokens,
        cachedInputTokens = inputTokensDetails?.cachedTokens ?: 0,
        outputTokens = outputTokens,
        reasoningOutputTokens = outputTokensDetails?.reasoningTokens ?: 0,
        totalTokens = totalTokens
    )
}

@Serializable
private data class SseEvent(
    val type: String,
    val response: JsonElement? = null,
    val item: JsonElement? = null,
    val delta: String? = null,
    @kotlinx.serialization.SerialName("summary_index")
    val summaryIndex: Long? = null,
    @kotlinx.serialization.SerialName("content_index")
    val contentIndex: Long? = null
)

/**
 * Process SSE stream for Responses API format.
 */
internal suspend fun processSse(
    response: HttpResponse,
    channel: Channel<Result<ResponseEvent>>,
    idleTimeout: Duration,
    telemetry: SseTelemetry?
) {
    val bodyChannel = response.bodyAsChannel()
    var responseCompleted: ResponseCompleted? = null
    var responseError: ApiError? = null
    val json = Json { ignoreUnknownKeys = true }
    val timeSource = TimeSource.Monotonic

    while (!bodyChannel.isClosedForRead) {
        val start = timeSource.markNow()

        // Read with timeout
        val line = withTimeoutOrNull(idleTimeout) {
            readSseLine(bodyChannel)
        }

        val elapsed = start.elapsedNow()
        telemetry?.onSsePoll(line != null, elapsed)

        if (line == null) {
            // Timeout
            channel.send(Result.failure(ApiError.Stream("idle timeout waiting for SSE")))
            return
        }

        // Parse SSE line
        val (eventType, data) = parseSseLine(line) ?: continue

        if (data.isBlank()) continue

        // Parse JSON
        val event = try {
            json.decodeFromString<SseEvent>(data)
        } catch (e: Exception) {
            // Skip malformed events
            continue
        }

        when (event.type) {
            "response.output_item.done" -> {
                val itemVal = event.item ?: continue
                val item = try {
                    json.decodeFromJsonElement<ResponseItem>(itemVal)
                } catch (e: Exception) {
                    continue
                }
                if (channel.trySend(Result.success(ResponseEvent.OutputItemDone(item))).isFailure) {
                    return
                }
            }

            "response.output_text.delta" -> {
                val delta = event.delta ?: continue
                if (channel.trySend(Result.success(ResponseEvent.OutputTextDelta(delta))).isFailure) {
                    return
                }
            }

            "response.reasoning_summary_text.delta" -> {
                val delta = event.delta ?: continue
                val summaryIndex = event.summaryIndex ?: continue
                if (channel.trySend(Result.success(ResponseEvent.ReasoningSummaryDelta(delta, summaryIndex))).isFailure) {
                    return
                }
            }

            "response.reasoning_text.delta" -> {
                val delta = event.delta ?: continue
                val contentIndex = event.contentIndex ?: continue
                if (channel.trySend(Result.success(ResponseEvent.ReasoningContentDelta(delta, contentIndex))).isFailure) {
                    return
                }
            }

            "response.created" -> {
                if (event.response != null) {
                    channel.trySend(Result.success(ResponseEvent.Created))
                }
            }

            "response.failed" -> {
                val respVal = event.response ?: continue
                responseError = ApiError.Stream("response.failed event received")

                val errorVal = (respVal as? JsonObject)?.get("error")
                if (errorVal != null) {
                    try {
                        val error = json.decodeFromJsonElement<SseError>(errorVal)
                        responseError = when {
                            isContextWindowError(error) -> ApiError.ContextWindowExceeded()
                            isQuotaExceededError(error) -> ApiError.QuotaExceeded()
                            isUsageNotIncluded(error) -> ApiError.UsageNotIncluded()
                            else -> {
                                val delay = tryParseRetryAfter(error)
                                val message = error.message ?: ""
                                ApiError.Retryable(message, delay)
                            }
                        }
                    } catch (e: Exception) {
                        // Keep the default error
                    }
                }
            }

            "response.completed" -> {
                val respVal = event.response ?: continue
                try {
                    responseCompleted = json.decodeFromJsonElement<ResponseCompleted>(respVal)
                } catch (e: Exception) {
                    responseError = ApiError.Stream("failed to parse ResponseCompleted: ${e.message}")
                }
            }

            "response.output_item.added" -> {
                val itemVal = event.item ?: continue
                val item = try {
                    json.decodeFromJsonElement<ResponseItem>(itemVal)
                } catch (e: Exception) {
                    continue
                }
                if (channel.trySend(Result.success(ResponseEvent.OutputItemAdded(item))).isFailure) {
                    return
                }
            }

            "response.reasoning_summary_part.added" -> {
                val summaryIndex = event.summaryIndex ?: continue
                if (channel.trySend(Result.success(ResponseEvent.ReasoningSummaryPartAdded(summaryIndex))).isFailure) {
                    return
                }
            }
        }
    }

    // Stream ended
    when {
        responseCompleted != null -> {
            val rc = responseCompleted!!
            val event = ResponseEvent.Completed(
                responseId = rc.id,
                tokenUsage = rc.usage?.toTokenUsage()
            )
            channel.send(Result.success(event))
        }
        responseError != null -> {
            channel.send(Result.failure(responseError!!))
        }
        else -> {
            channel.send(Result.failure(ApiError.Stream("stream closed before response.completed")))
        }
    }
}

// ============================================================================
// Internal SSE parsing for Chat Completions API
// ============================================================================

/**
 * Process SSE stream for Chat Completions API format.
 */
internal suspend fun processChatSse(
    response: HttpResponse,
    channel: Channel<Result<ResponseEvent>>,
    idleTimeout: Duration,
    telemetry: SseTelemetry?
) {
    val bodyChannel = response.bodyAsChannel()
    val json = Json { ignoreUnknownKeys = true }
    val timeSource = TimeSource.Monotonic

    // State for accumulating tool calls
    data class ToolCallState(
        var name: String? = null,
        var arguments: String = ""
    )

    val toolCalls = mutableMapOf<String, ToolCallState>()
    val toolCallOrder = mutableListOf<String>()
    var assistantItem: ResponseItem.Message? = null
    var reasoningItem: ResponseItem.Reasoning? = null
    var completedSent = false

    while (!bodyChannel.isClosedForRead) {
        val start = timeSource.markNow()

        val line = withTimeoutOrNull(idleTimeout) {
            readSseLine(bodyChannel)
        }

        val elapsed = start.elapsedNow()
        telemetry?.onSsePoll(line != null, elapsed)

        if (line == null) {
            // Timeout
            channel.send(Result.failure(ApiError.Stream("idle timeout waiting for SSE")))
            return
        }

        val (_, data) = parseSseLine(line) ?: continue

        if (data.isBlank()) continue

        val value = try {
            json.parseToJsonElement(data).jsonObject
        } catch (e: Exception) {
            continue
        }

        val choices = value["choices"]?.jsonArray ?: continue

        for (choice in choices) {
            val choiceObj = choice.jsonObject

            // Process delta
            val delta = choiceObj["delta"]?.jsonObject
            if (delta != null) {
                // Handle reasoning
                val reasoning = delta["reasoning"]
                if (reasoning != null) {
                    val text = when {
                        reasoning is JsonPrimitive && reasoning.isString -> reasoning.content
                        reasoning is JsonObject -> reasoning["text"]?.jsonPrimitive?.contentOrNull
                            ?: reasoning["content"]?.jsonPrimitive?.contentOrNull
                        else -> null
                    }
                    if (text != null) {
                        appendReasoningText(channel, reasoningItem, text).also { reasoningItem = it }
                    }
                }

                // Handle content
                val content = delta["content"]
                if (content != null) {
                    when {
                        content is JsonArray -> {
                            for (item in content) {
                                val text = item.jsonObject["text"]?.jsonPrimitive?.contentOrNull
                                if (text != null) {
                                    appendAssistantText(channel, assistantItem, text).also { assistantItem = it }
                                }
                            }
                        }
                        content is JsonPrimitive && content.isString -> {
                            appendAssistantText(channel, assistantItem, content.content).also { assistantItem = it }
                        }
                    }
                }

                // Handle tool calls
                val toolCallsVal = delta["tool_calls"]?.jsonArray
                if (toolCallsVal != null) {
                    for (toolCall in toolCallsVal) {
                        val tcObj = toolCall.jsonObject
                        val id = tcObj["id"]?.jsonPrimitive?.contentOrNull
                            ?: "tool-call-${toolCallOrder.size}"

                        val callState = toolCalls.getOrPut(id) { ToolCallState() }
                        if (id !in toolCallOrder) {
                            toolCallOrder.add(id)
                        }

                        val func = tcObj["function"]?.jsonObject
                        if (func != null) {
                            func["name"]?.jsonPrimitive?.contentOrNull?.let { callState.name = it }
                            func["arguments"]?.jsonPrimitive?.contentOrNull?.let { callState.arguments += it }
                        }
                    }
                }
            }

            // Process message (non-streaming format)
            val message = choiceObj["message"]?.jsonObject
            if (message != null) {
                val reasoning = message["reasoning"]
                if (reasoning != null) {
                    val text = when {
                        reasoning is JsonPrimitive && reasoning.isString -> reasoning.content
                        reasoning is JsonObject -> reasoning["text"]?.jsonPrimitive?.contentOrNull
                            ?: reasoning["content"]?.jsonPrimitive?.contentOrNull
                        else -> null
                    }
                    if (text != null) {
                        appendReasoningText(channel, reasoningItem, text).also { reasoningItem = it }
                    }
                }
            }

            // Handle finish reason
            val finishReason = choiceObj["finish_reason"]?.jsonPrimitive?.contentOrNull

            when (finishReason) {
                "stop" -> {
                    reasoningItem?.let {
                        channel.trySend(Result.success(ResponseEvent.OutputItemDone(it)))
                    }
                    assistantItem?.let {
                        channel.trySend(Result.success(ResponseEvent.OutputItemDone(it)))
                    }
                    if (!completedSent) {
                        channel.trySend(Result.success(ResponseEvent.Completed(
                            responseId = "",
                            tokenUsage = null
                        )))
                        completedSent = true
                    }
                    reasoningItem = null
                    assistantItem = null
                }

                "length" -> {
                    channel.send(Result.failure(ApiError.ContextWindowExceeded()))
                    return
                }

                "tool_calls" -> {
                    reasoningItem?.let {
                        channel.trySend(Result.success(ResponseEvent.OutputItemDone(it)))
                    }
                    reasoningItem = null

                    for (callId in toolCallOrder) {
                        val state = toolCalls.remove(callId) ?: ToolCallState()
                        val item = ResponseItem.FunctionCall(
                            id = null,
                            name = state.name ?: "",
                            arguments = state.arguments,
                            callId = callId
                        )
                        channel.trySend(Result.success(ResponseEvent.OutputItemDone(item)))
                    }
                    toolCallOrder.clear()
                }
            }
        }
    }

    // Stream ended normally
    reasoningItem?.let {
        channel.trySend(Result.success(ResponseEvent.OutputItemDone(it)))
    }
    assistantItem?.let {
        channel.trySend(Result.success(ResponseEvent.OutputItemDone(it)))
    }
    if (!completedSent) {
        channel.trySend(Result.success(ResponseEvent.Completed(
            responseId = "",
            tokenUsage = null
        )))
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/**
 * Read a single SSE line from the channel.
 * SSE lines are separated by \n\n
 */
private suspend fun readSseLine(channel: ByteReadChannel): String {
    val builder = StringBuilder()
    var sawNewline = false

    while (true) {
        val byte = channel.readByte()
        val char = byte.toInt().toChar()

        if (char == '\n') {
            if (sawNewline) {
                // Double newline - end of event
                break
            }
            sawNewline = true
            builder.append(char)
        } else {
            sawNewline = false
            builder.append(char)
        }
    }

    return builder.toString()
}

/**
 * Parse an SSE line into event type and data.
 * Returns null if the line is not a valid SSE line.
 */
private fun parseSseLine(line: String): Pair<String, String>? {
    var eventType = ""
    var data = ""

    for (part in line.split('\n')) {
        when {
            part.startsWith("event:") -> {
                eventType = part.removePrefix("event:").trim()
            }
            part.startsWith("data:") -> {
                data = part.removePrefix("data:").trim()
            }
        }
    }

    if (eventType.isEmpty() && data.isEmpty()) {
        return null
    }

    return Pair(eventType, data)
}

/**
 * Append text to assistant message, creating if needed.
 */
private suspend fun appendAssistantText(
    channel: Channel<Result<ResponseEvent>>,
    current: ResponseItem.Message?,
    text: String
): ResponseItem.Message {
    val item = if (current == null) {
        val newItem = ResponseItem.Message(
            id = null,
            role = "assistant",
            content = mutableListOf()
        )
        channel.trySend(Result.success(ResponseEvent.OutputItemAdded(newItem)))
        newItem
    } else {
        current
    }

    (item.content as MutableList).add(ContentItem.OutputText(text))
    channel.trySend(Result.success(ResponseEvent.OutputTextDelta(text)))

    return item
}

/**
 * Append text to reasoning item, creating if needed.
 */
private suspend fun appendReasoningText(
    channel: Channel<Result<ResponseEvent>>,
    current: ResponseItem.Reasoning?,
    text: String
): ResponseItem.Reasoning {
    val item = if (current == null) {
        val newItem = ResponseItem.Reasoning(
            id = "",
            summary = emptyList(),
            content = mutableListOf(),
            encryptedContent = null
        )
        channel.trySend(Result.success(ResponseEvent.OutputItemAdded(newItem)))
        newItem
    } else {
        current
    }

    val contentList = item.content as? MutableList ?: mutableListOf()
    val contentIndex = contentList.size.toLong()
    contentList.add(ReasoningItemContent.ReasoningText(text))

    channel.trySend(Result.success(ResponseEvent.ReasoningContentDelta(text, contentIndex)))

    return item.copy(content = contentList)
}

/**
 * Check if error is context window exceeded.
 */
private fun isContextWindowError(error: SseError): Boolean {
    return error.code == "context_length_exceeded"
}

/**
 * Check if error is quota exceeded.
 */
private fun isQuotaExceededError(error: SseError): Boolean {
    return error.code == "insufficient_quota"
}

/**
 * Check if error is usage not included.
 */
private fun isUsageNotIncluded(error: SseError): Boolean {
    return error.code == "usage_not_included"
}

/**
 * Try to parse retry-after duration from rate limit error message.
 */
private fun tryParseRetryAfter(error: SseError): Duration? {
    if (error.code != "rate_limit_exceeded") {
        return null
    }

    val message = error.message ?: return null

    // Pattern: "try again in X.XXs" or "try again in Xms" or "try again in X seconds"
    val regex = Regex("""(?i)try again in\s*(\d+(?:\.\d+)?)\s*(s|ms|seconds?)""")
    val match = regex.find(message) ?: return null

    val value = match.groupValues[1].toDoubleOrNull() ?: return null
    val unit = match.groupValues[2].lowercase()

    return when {
        unit == "s" || unit.startsWith("second") -> (value * 1000).toLong().milliseconds
        unit == "ms" -> value.toLong().milliseconds
        else -> null
    }
}
