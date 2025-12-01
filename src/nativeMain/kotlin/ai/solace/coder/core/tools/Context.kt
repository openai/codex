// port-lint: source core/src/tools/context.rs
package ai.solace.coder.core.tools

import ai.solace.coder.core.session.Session
import ai.solace.coder.core.session.TurnContext
import ai.solace.coder.core.turn_diff_tracker.TurnDiffTracker
import ai.solace.coder.protocol.FunctionCallOutputContentItem
import ai.solace.coder.protocol.FunctionCallOutputPayload
import ai.solace.coder.protocol.ResponseInputItem
import ai.solace.coder.protocol.ShellToolCallParams
import ai.solace.coder.protocol.CallToolResult
import kotlinx.coroutines.sync.Mutex

typealias SharedTurnDiffTracker = Mutex // Placeholder or actual type if available

data class ToolInvocation(
    val session: Session,
    val turn: TurnContext,
    val tracker: SharedTurnDiffTracker,
    val callId: String,
    val toolName: String,
    val payload: ToolPayload
)

sealed class ToolPayload {
    data class Function(val arguments: String) : ToolPayload()
    data class Custom(val input: String) : ToolPayload()
    data class LocalShell(val params: ShellToolCallParams) : ToolPayload()
    data class UnifiedExec(val arguments: String) : ToolPayload()
    data class Mcp(
        val server: String,
        val tool: String,
        val rawArguments: String
    ) : ToolPayload()

    fun logPayload(): String {
        return when (this) {
            is Function -> arguments
            is Custom -> input
            is LocalShell -> params.command.joinToString(" ")
            is UnifiedExec -> arguments
            is Mcp -> rawArguments
        }
    }
}

sealed class ToolOutput {
    data class Function(
        val content: String,
        val contentItems: List<FunctionCallOutputContentItem>? = null,
        val success: Boolean? = null
    ) : ToolOutput()

    data class Mcp(
        val result: Result<CallToolResult>
    ) : ToolOutput()

    fun logPreview(): String {
        return when (this) {
            is Function -> telemetryPreview(content)
            is Mcp -> result.toString()
        }
    }

    fun successForLogging(): Boolean {
        return when (this) {
            is Function -> success ?: true
            is Mcp -> result.isSuccess
        }
    }

    fun intoResponse(callId: String, payload: ToolPayload): ResponseInputItem {
        return when (this) {
            is Function -> {
                if (payload is ToolPayload.Custom) {
                    ResponseInputItem.CustomToolCallOutput(
                        callId = callId,
                        output = content
                    )
                } else {
                    ResponseInputItem.FunctionCallOutput(
                        callId = callId,
                        output = FunctionCallOutputPayload(
                            content = content,
                            contentItems = contentItems,
                            success = success
                        )
                    )
                }
            }
            is Mcp -> ResponseInputItem.McpToolCallOutput(
                callId = callId,
                result = result
            )
        }
    }
}

fun telemetryPreview(content: String): String {
    // Kotlin implementation of take_bytes_at_char_boundary logic
    // For simplicity, we'll just take characters for now, but ideally should respect byte limit
    // TELEMETRY_PREVIEW_MAX_BYTES is defined in Tools.kt (mod.rs)
    
    val truncatedSlice = if (content.length > TELEMETRY_PREVIEW_MAX_BYTES) {
        content.substring(0, TELEMETRY_PREVIEW_MAX_BYTES) // Approximation
    } else {
        content
    }
    
    val truncatedByBytes = truncatedSlice.length < content.length

    val lines = truncatedSlice.lines()
    val previewLines = lines.take(TELEMETRY_PREVIEW_MAX_LINES)
    val truncatedByLines = lines.size > TELEMETRY_PREVIEW_MAX_LINES

    if (!truncatedByBytes && !truncatedByLines) {
        return content
    }

    val preview = StringBuilder()
    previewLines.forEachIndexed { index, line ->
        if (index > 0) preview.append("\n")
        preview.append(line)
    }

    if (preview.length < truncatedSlice.length && truncatedSlice[preview.length] == '\n') {
        preview.append("\n")
    }

    if (preview.isNotEmpty() && !preview.endsWith("\n")) {
        preview.append("\n")
    }
    preview.append(TELEMETRY_PREVIEW_TRUNCATION_NOTICE)

    return preview.toString()
}
