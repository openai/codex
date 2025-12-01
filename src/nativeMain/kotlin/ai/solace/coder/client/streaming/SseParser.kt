// port-lint: source codex-api/src/sse/responses.rs
package ai.solace.coder.client.streaming

import ai.solace.coder.protocol.ResponseEvent
import ai.solace.coder.protocol.ResponseItem
import ai.solace.coder.protocol.TokenUsage
import ai.solace.coder.protocol.RateLimitSnapshot
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * Parser for Server-Sent Events (SSE) format.
 * Parses SSE streams from the Codex API /responses endpoint.
 * 
 * Maps to Rust's SSE parsing in eventsource-stream.
 */
class SseParser {
    private val json = Json {
        ignoreUnknownKeys = true
        isLenient = true
    }
    
    /**
     * Parse an SSE stream into response events.
     */
    fun parse(sseData: String): List<ResponseEvent> {
        val events = mutableListOf<ResponseEvent>()
        val lines = sseData.lines()
        
        var currentEvent: String? = null
        var currentData = StringBuilder()
        
        for (line in lines) {
            when {
                line.startsWith("event:") -> {
                    // Flush previous event if any
                    if (currentEvent != null && currentData.isNotEmpty()) {
                        parseEvent(currentEvent, currentData.toString())?.let { events.add(it) }
                    }
                    
                    currentEvent = line.substring(6).trim()
                    currentData = StringBuilder()
                }
                
                line.startsWith("data:") -> {
                    val data = line.substring(5).trim()
                    if (currentData.isNotEmpty()) {
                        currentData.append("\n")
                    }
                    currentData.append(data)
                }
                
                line.isEmpty() -> {
                    // End of event
                    if (currentEvent != null && currentData.isNotEmpty()) {
                        parseEvent(currentEvent, currentData.toString())?.let { events.add(it) }
                        currentEvent = null
                        currentData = StringBuilder()
                    }
                }
            }
        }
        
        // Flush final event if any
        if (currentEvent != null && currentData.isNotEmpty()) {
            parseEvent(currentEvent, currentData.toString())?.let { events.add(it) }
        }
        
        return events
    }
    
    /**
     * Parse a single SSE event into a ResponseEvent.
     */
    private fun parseEvent(eventType: String, data: String): ResponseEvent? {
        return try {
            when (eventType) {
                "response.created" -> ResponseEvent.Created
                
                "response.output_item.added" -> {
                    val item = json.decodeFromString<ResponseItem>(data)
                    ResponseEvent.OutputItemAdded(item)
                }
                
                "response.output_item.done" -> {
                    val item = json.decodeFromString<ResponseItem>(data)
                    ResponseEvent.OutputItemDone(item)
                }
                
                "response.output.text.delta" -> {
                    val jsonElement = json.parseToJsonElement(data)
                    val delta = jsonElement.jsonObject["delta"]?.jsonPrimitive?.content ?: ""
                    ResponseEvent.OutputTextDelta(delta)
                }
                
                "response.reasoning.summary.delta" -> {
                    val jsonElement = json.parseToJsonElement(data)
                    val obj = jsonElement.jsonObject
                    val delta = obj["delta"]?.jsonPrimitive?.content ?: ""
                    val summaryIndex = obj["summary_index"]?.jsonPrimitive?.content?.toLongOrNull() ?: 0L
                    ResponseEvent.ReasoningSummaryDelta(delta, summaryIndex)
                }
                
                "response.reasoning.summary.part.added" -> {
                    val jsonElement = json.parseToJsonElement(data)
                    val summaryIndex = jsonElement.jsonObject["summary_index"]?.jsonPrimitive?.content?.toLongOrNull() ?: 0L
                    ResponseEvent.ReasoningSummaryPartAdded(summaryIndex)
                }
                
                "response.reasoning.content.delta" -> {
                    val jsonElement = json.parseToJsonElement(data)
                    val obj = jsonElement.jsonObject
                    val delta = obj["delta"]?.jsonPrimitive?.content ?: ""
                    val contentIndex = obj["content_index"]?.jsonPrimitive?.content?.toLongOrNull() ?: 0L
                    ResponseEvent.ReasoningContentDelta(delta, contentIndex)
                }
                
                "rate_limits" -> {
                    val snapshot = json.decodeFromString<RateLimitSnapshot>(data)
                    ResponseEvent.RateLimits(snapshot)
                }
                
                "response.completed" -> {
                    val jsonElement = json.parseToJsonElement(data)
                    val obj = jsonElement.jsonObject
                    val responseId = obj["response_id"]?.jsonPrimitive?.content ?: ""
                    val tokenUsage = obj["usage"]?.let { 
                        json.decodeFromJsonElement(TokenUsage.serializer(), it)
                    }
                    ResponseEvent.Completed(responseId, tokenUsage)
                }
                
                else -> {
                    // Unknown event type, skip
                    null
                }
            }
        } catch (e: Exception) {
            // Log parse error but continue processing stream
            // Ported from Rust codex-rs/core/src/client.rs map_response_stream error handling
            println("WARN: SSE parse failed for event type '$eventType': ${e.message}")
            null
        }
    }
}

/**
 * SSE event from the stream.
 */
data class SseEvent(
    val eventType: String,
    val data: String,
    val id: String? = null,
    val retry: Long? = null
)