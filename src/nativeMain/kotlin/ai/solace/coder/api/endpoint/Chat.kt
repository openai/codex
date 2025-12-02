// port-lint: source codex-rs/codex-api/src/endpoint/chat.rs
package ai.solace.coder.api.endpoint

import ai.solace.coder.api.AuthProvider
import ai.solace.coder.api.common.Prompt
import ai.solace.coder.api.common.ResponseEvent
import ai.solace.coder.api.common.ResponseStream
import ai.solace.coder.api.provider.Provider
import ai.solace.coder.api.provider.WireApi
import ai.solace.coder.api.requests.ChatRequest
import ai.solace.coder.api.requests.ChatRequestBuilder
import ai.solace.coder.api.telemetry.RequestTelemetry
import ai.solace.coder.api.telemetry.SseTelemetry
import io.ktor.client.*
import kotlinx.serialization.json.JsonElement

/** Client for Chat Completions endpoint. */
class ChatClient<A : AuthProvider>(
    httpClient: HttpClient,
    provider: Provider,
    auth: A,
) {
    private val streaming: StreamingClient<A> = StreamingClient(httpClient, provider, auth)

    fun withTelemetry(
        request: RequestTelemetry?,
        sse: SseTelemetry?,
    ): ChatClient<A> {
        streaming.withTelemetry(request, sse)
        return this
    }

    suspend fun streamRequest(request: ChatRequest): Result<ResponseStream> {
        return stream(request.body, request.configureHeaders)
    }

    suspend fun streamPrompt(
        model: String,
        prompt: Prompt,
        conversationId: String?,
        sessionSource: ai.solace.coder.protocol.SessionSource?,
    ): Result<ResponseStream> {
        val request = ChatRequestBuilder(model, prompt.instructions, prompt.input, prompt.tools)
            .conversationId(conversationId)
            .sessionSource(sessionSource)
            .build(streaming.provider())
            .getOrElse { return Result.failure(it) }
        return streamRequest(request)
    }

    private fun path(): String {
        return when (streaming.provider().wire) {
            WireApi.Chat -> "chat/completions"
            else -> "responses"
        }
    }

    suspend fun stream(
        body: JsonElement,
        configureExtraHeaders: io.ktor.client.request.HttpRequestBuilder.() -> Unit,
    ): Result<ResponseStream> {
        // TODO: Implement spawnChatStream once SSE parsing is ported
        return streaming.stream(path(), body, configureExtraHeaders) { _, _, _ ->
            TODO("spawnChatStream not yet implemented")
        }
    }
}

/** Aggregation mode for stream processing. */
enum class AggregateMode {
    AggregatedOnly,
    Streaming,
}

/**
 * Stream adapter that merges token deltas into a single assistant message per turn.
 * Mirrors Rust's AggregatedStream impl Stream for AggregatedStream.
 */
class AggregatedStream private constructor(
    private val inner: ResponseStream,
    private val mode: AggregateMode,
) {
    private val cumulative = StringBuilder()
    private val cumulativeReasoning = StringBuilder()
    private val pending = ArrayDeque<ResponseEvent>()

    /**
     * Poll the next event from the aggregated stream.
     * This implements the full Rust poll_next logic for event aggregation.
     */
    suspend fun pollNext(): Result<ResponseEvent?> {
        // Return pending events first
        pending.firstOrNull()?.let { event ->
            pending.removeFirst()
            return Result.success(event)
        }

        // Poll inner stream in a loop, aggregating as we go
        while (true) {
            val result = inner.next()

            // Handle errors and end-of-stream
            if (result.isFailure) {
                return result
            }

            val event = result.getOrNull() ?: return Result.success(null)

            when (event) {
                is ResponseEvent.OutputItemDone -> {
                    val item = event.item
                    val isAssistantMessage = item is ai.solace.coder.protocol.ResponseItem.Message && item.role == "assistant"

                    if (isAssistantMessage) {
                        when (mode) {
                            AggregateMode.AggregatedOnly -> {
                                // Accumulate text from first message with OutputText content
                                if (cumulative.isEmpty()) {
                                    item.content.firstOrNull { it is ai.solace.coder.protocol.ContentItem.OutputText }
                                        ?.let { contentItem ->
                                            if (contentItem is ai.solace.coder.protocol.ContentItem.OutputText) {
                                                cumulative.append(contentItem.text)
                                            }
                                        }
                                }
                                continue // Don't emit, keep looping
                            }
                            AggregateMode.Streaming -> {
                                // In streaming mode, emit the item if we haven't accumulated anything
                                if (cumulative.isEmpty()) {
                                    return Result.success(event)
                                } else {
                                    continue // Skip this item, we're aggregating
                                }
                            }
                        }
                    }

                    // Non-assistant messages pass through
                    return Result.success(event)
                }

                is ResponseEvent.RateLimits -> {
                    return Result.success(event)
                }

                is ResponseEvent.Completed -> {
                    var emittedAny = false

                    // Emit aggregated reasoning if we accumulated any
                    if (cumulativeReasoning.isNotEmpty()) {
                        val aggregatedReasoning = ai.solace.coder.protocol.ResponseItem.Reasoning(
                            id = "",
                            summary = emptyList(),
                            content = listOf(ai.solace.coder.protocol.ReasoningItemContent.ReasoningText(
                                text = cumulativeReasoning.toString()
                            )),
                            encryptedContent = null
                        )
                        pending.add(ResponseEvent.OutputItemDone(aggregatedReasoning))
                        cumulativeReasoning.clear()
                        emittedAny = true
                    }

                    // Emit aggregated message if we accumulated any
                    if (cumulative.isNotEmpty()) {
                        val aggregatedMessage = ai.solace.coder.protocol.ResponseItem.Message(
                            role = "assistant",
                            content = listOf(ai.solace.coder.protocol.ContentItem.OutputText(text = cumulative.toString())),
                            id = null
                        )
                        pending.add(ResponseEvent.OutputItemDone(aggregatedMessage))
                        cumulative.clear()
                        emittedAny = true
                    }

                    // Add the completion event at the end
                    if (emittedAny) {
                        pending.add(event)
                        // Return the first pending event
                        return Result.success(pending.removeFirst())
                    }

                    return Result.success(event)
                }

                is ResponseEvent.Created -> {
                    continue // Skip Created events
                }

                is ResponseEvent.OutputTextDelta -> {
                    cumulative.append(event.delta)
                    if (mode == AggregateMode.Streaming) {
                        return Result.success(event)
                    } else {
                        continue // Accumulate but don't emit
                    }
                }

                is ResponseEvent.ReasoningContentDelta -> {
                    cumulativeReasoning.append(event.delta)
                    if (mode == AggregateMode.Streaming) {
                        return Result.success(event)
                    } else {
                        continue // Accumulate but don't emit
                    }
                }

                is ResponseEvent.ReasoningSummaryDelta -> {
                    continue // Skip summary deltas
                }

                is ResponseEvent.ReasoningSummaryPartAdded -> {
                    continue // Skip summary part additions
                }

                is ResponseEvent.OutputItemAdded -> {
                    return Result.success(event)
                }
            }
        }
    }

    companion object {
        fun new(inner: ResponseStream, mode: AggregateMode): AggregatedStream {
            return AggregatedStream(inner, mode)
        }
    }
}

/**
 * Extension functions for ResponseStream aggregation.
 * Mirrors Rust's AggregateStreamExt trait.
 */
fun ResponseStream.aggregate(): AggregatedStream {
    return AggregatedStream.new(this, AggregateMode.AggregatedOnly)
}

fun ResponseStream.streamingMode(): ResponseStream {
    return this
}
