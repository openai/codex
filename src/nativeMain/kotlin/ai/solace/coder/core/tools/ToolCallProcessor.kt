package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.core.session.Session
import ai.solace.coder.core.session.TurnContext
import ai.solace.coder.protocol.models.CallToolResult
import ai.solace.coder.protocol.models.FunctionCallOutputPayload
import ai.solace.coder.protocol.models.ResponseInputItem
import ai.solace.coder.protocol.models.ResponseItem

/**
 * Configuration for tool call processing.
 *
 * Ported from Rust codex-rs/core/src/response_processing.rs
 */
data class ToolCallProcessorConfig(
    val enableParallelExecution: Boolean = false,
    val maxConcurrentCalls: Int = 1,
    val defaultTimeoutMs: Long = 60000L
)

/**
 * A response item paired with its optional response input.
 *
 * When the model is prompted, it returns a stream of events. Some of these
 * events map to a `ResponseItem`. A `ResponseItem` may need to be
 * "handled" such that it produces a `ResponseInputItem` that needs to be
 * sent back to the model on the next turn.
 *
 * Ported from Rust codex-rs/core/src/codex.rs ProcessedResponseItem
 */
data class ProcessedResponseItem(
    val item: ResponseItem,
    val response: ResponseInputItem?
)

/**
 * Result of processing response items.
 *
 * Contains both the items to send back to the model and all items to record
 * in conversation history.
 */
data class ProcessItemsResult(
    val responses: List<ResponseInputItem>,
    val itemsToRecord: List<ResponseItem>
)

/**
 * Processes function calls from model responses.
 *
 * Ported from Rust codex-rs/core/src/response_processing.rs
 *
 * Implemented features:
 * - [x] process_items() - full response item processing
 * - [x] Response to input item conversion
 * - [x] MCP tool call result handling
 *
 * TODO: Port remaining features:
 * - [ ] Parallel tool execution with FuturesOrdered
 * - [ ] Full streaming integration
 */
class ToolCallProcessor(
    private val config: ToolCallProcessorConfig = ToolCallProcessorConfig()
) {

    /**
     * Process streamed `ResponseItem`s from the model into the pair of:
     * - items we should record in conversation history; and
     * - `ResponseInputItem`s to send back to the model on the next turn.
     *
     * Ported from Rust codex-rs/core/src/response_processing.rs process_items()
     */
    suspend fun processItems(
        processedItems: List<ProcessedResponseItem>,
        session: Session,
        turnContext: TurnContext
    ): ProcessItemsResult {
        val outputsToRecord = mutableListOf<ResponseItem>()
        val newInputsToRecord = mutableListOf<ResponseItem>()
        val responses = mutableListOf<ResponseInputItem>()

        for (processedResponseItem in processedItems) {
            val item = processedResponseItem.item
            val response = processedResponseItem.response

            if (response != null) {
                responses.add(response)
            }

            when (response) {
                is ResponseInputItem.FunctionCallOutput -> {
                    newInputsToRecord.add(
                        ResponseItem.FunctionCallOutput(
                            call_id = response.call_id,
                            output = response.output
                        )
                    )
                }

                is ResponseInputItem.CustomToolCallOutput -> {
                    newInputsToRecord.add(
                        ResponseItem.CustomToolCallOutput(
                            call_id = response.call_id,
                            output = response.output
                        )
                    )
                }

                is ResponseInputItem.McpToolCallOutput -> {
                    val output = when {
                        response.result.isSuccess && response.result.value != null -> {
                            FunctionCallOutputPayload.fromCallToolResult(response.result.value)
                        }
                        response.result.isSuccess -> {
                            FunctionCallOutputPayload(
                                content = "null result",
                                success = false
                            )
                        }
                        else -> {
                            val error = response.result.error ?: "Unknown error"
                            FunctionCallOutputPayload(
                                content = error,
                                success = false
                            )
                        }
                    }
                    newInputsToRecord.add(
                        ResponseItem.FunctionCallOutput(
                            call_id = response.call_id,
                            output = output
                        )
                    )
                }

                null -> {
                    // No response to record
                }

                else -> {
                    // Unexpected response item type - log warning
                    println("WARN: Unexpected response item: $item with response: $response")
                }
            }

            outputsToRecord.add(item)
        }

        val allItemsToRecord = outputsToRecord + newInputsToRecord

        // Only attempt to record if there is something to record
        if (allItemsToRecord.isNotEmpty()) {
            session.recordConversationItems(turnContext, allItemsToRecord)
        }

        return ProcessItemsResult(
            responses = responses,
            itemsToRecord = allItemsToRecord
        )
    }

    /**
     * Create a processed response item for a tool call result.
     */
    fun createToolCallProcessedItem(
        item: ResponseItem,
        callId: String,
        output: FunctionCallOutputPayload
    ): ProcessedResponseItem {
        return ProcessedResponseItem(
            item = item,
            response = ResponseInputItem.FunctionCallOutput(
                call_id = callId,
                output = output
            )
        )
    }

    /**
     * Create a processed response item for a non-tool response.
     */
    fun createNonToolProcessedItem(item: ResponseItem): ProcessedResponseItem {
        return ProcessedResponseItem(
            item = item,
            response = null
        )
    }

    /**
     * Create an error response for a failed tool call.
     */
    fun createErrorResponse(callId: String, errorMessage: String): ResponseInputItem {
        return ResponseInputItem.FunctionCallOutput(
            call_id = callId,
            output = FunctionCallOutputPayload(
                content = errorMessage,
                success = false
            )
        )
    }

    /**
     * Get processor statistics.
     */
    fun getStatistics(): ToolCallProcessorStats {
        return ToolCallProcessorStats(
            maxConcurrentCalls = config.maxConcurrentCalls,
            parallelExecutionEnabled = config.enableParallelExecution,
            defaultTimeoutMs = config.defaultTimeoutMs
        )
    }
}

/**
 * Statistics about the tool call processor.
 */
data class ToolCallProcessorStats(
    val maxConcurrentCalls: Int,
    val parallelExecutionEnabled: Boolean,
    val defaultTimeoutMs: Long
)
