// port-lint: source core/src/response_processing.rs
package ai.solace.coder.core

import ai.solace.coder.core.session.Session
import ai.solace.coder.core.session.TurnContext
import ai.solace.coder.protocol.FunctionCallOutputPayload
import ai.solace.coder.protocol.ResponseInputItem
import ai.solace.coder.protocol.ResponseItem

data class ToolCallProcessorConfig(
    val enableParallelExecution: Boolean = false,
    val maxConcurrentCalls: Int = 1,
    val defaultTimeoutMs: Long = 60000L
)

data class ProcessedResponseItem(
    val item: ResponseItem,
    val response: ResponseInputItem?
)

data class ProcessItemsResult(
    val responses: List<ResponseInputItem>,
    val itemsToRecord: List<ResponseItem>
)

class ToolCallProcessor(
    private val config: ToolCallProcessorConfig = ToolCallProcessorConfig()
) {
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
                            callId = response.callId,
                            output = response.output
                        )
                    )
                }
                is ResponseInputItem.CustomToolCallOutput -> {
                    newInputsToRecord.add(
                        ResponseItem.CustomToolCallOutput(
                            callId = response.callId,
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
                            callId = response.callId,
                            output = output
                        )
                    )
                }
                null -> {}
                else -> {
                    println("WARN: Unexpected response item: $item with response: $response")
                }
            }

            outputsToRecord.add(item)
        }

        val allItemsToRecord = outputsToRecord + newInputsToRecord

        if (allItemsToRecord.isNotEmpty()) {
            session.recordConversationItems(turnContext, allItemsToRecord)
        }

        return ProcessItemsResult(
            responses = responses,
            itemsToRecord = allItemsToRecord
        )
    }
}
