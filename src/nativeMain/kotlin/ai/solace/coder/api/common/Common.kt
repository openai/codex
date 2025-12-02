// port-lint: source codex-rs/codex-api/src/common.rs
package ai.solace.coder.api.common

import ai.solace.coder.protocol.ResponseItem
import ai.solace.coder.protocol.TokenUsage
import ai.solace.coder.protocol.RateLimitSnapshot
import ai.solace.coder.protocol.ReasoningEffortConfig
import ai.solace.coder.protocol.ReasoningSummaryConfig
import ai.solace.coder.protocol.Verbosity
import kotlinx.serialization.json.JsonElement

// Type alias matching Rust usage pattern
typealias VerbosityConfig = Verbosity

/**
 * Canonical prompt input for Chat and Responses endpoints.
 */
data class Prompt(
    val instructions: String,
    val input: List<ResponseItem>,
    val tools: List<JsonElement>,
    val parallelToolCalls: Boolean,
    val outputSchema: JsonElement?
)

/** Canonical input payload for the compaction endpoint. */
data class CompactionInput(
    val model: String,
    val input: List<ResponseItem>,
    val instructions: String,
)

/** Events emitted by streaming responses. */
sealed class ResponseEvent {
    data object Created : ResponseEvent()
    data class OutputItemDone(val item: ResponseItem) : ResponseEvent()
    data class OutputItemAdded(val item: ResponseItem) : ResponseEvent()
    data class Completed(val responseId: String, val tokenUsage: TokenUsage?) : ResponseEvent()
    data class OutputTextDelta(val delta: String) : ResponseEvent()
    data class ReasoningSummaryDelta(val delta: String, val summaryIndex: Long) : ResponseEvent()
    data class ReasoningContentDelta(val delta: String, val contentIndex: Long) : ResponseEvent()
    data class ReasoningSummaryPartAdded(val summaryIndex: Long) : ResponseEvent()
    data class RateLimits(val snapshot: RateLimitSnapshot) : ResponseEvent()
}

/** Reasoning config payload. */
data class Reasoning(
    val effort: ReasoningEffortConfig?,
    val summary: ReasoningSummaryConfig?,
)

/** Text formatting types used by OpenAI text controls. */
enum class TextFormatType { JsonSchema }

/** Controls JSON formatted output. */
data class TextFormat(
    val type: TextFormatType,
    val strict: Boolean,
    val schema: JsonElement,
    val name: String,
)

/** Controls the text field for Responses API. */
data class TextControls(
    val verbosity: OpenAiVerbosity?,
    val format: TextFormat?,
)

/** Verbosity mapping for OpenAI. */
enum class OpenAiVerbosity { Low, Medium, High }

fun openAiVerbosityConfig(v: VerbosityConfig): OpenAiVerbosity = when (v) {
    VerbosityConfig.Low -> OpenAiVerbosity.Low
    VerbosityConfig.Medium -> OpenAiVerbosity.Medium
    VerbosityConfig.High -> OpenAiVerbosity.High
}

/** Responses API request payload. */
data class ResponsesApiRequest(
    val model: String,
    val instructions: String,
    val input: List<ResponseItem>,
    val tools: List<JsonElement>,
    val toolChoice: String,
    val parallelToolCalls: Boolean,
    val reasoning: Reasoning?,
    val store: Boolean,
    val stream: Boolean,
    val include: List<String>,
    val promptCacheKey: String?,
    val text: TextControls?,
)

/** Create text param controls from verbosity and optional output schema. */
fun createTextParamForRequest(
    verbosity: VerbosityConfig?,
    outputSchema: JsonElement?,
): TextControls? {
    if (verbosity == null && outputSchema == null) return null
    val format = outputSchema?.let { schema ->
        TextFormat(
            type = TextFormatType.JsonSchema,
            strict = true,
            schema = schema,
            name = "codex_output_schema",
        )
    }
    return TextControls(
        verbosity = verbosity?.let { openAiVerbosityConfig(it) },
        format = format,
    )
}

/** Stream of response events. */
interface ResponseStream {
    /**
     * Receive the next event, or null if stream ended.
     * TODO: Replace with channel/Flow once concurrency primitives are selected.
     */
    suspend fun next(): Result<ResponseEvent?>
}

