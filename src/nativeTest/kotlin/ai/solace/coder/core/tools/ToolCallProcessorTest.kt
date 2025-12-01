package ai.solace.coder.core.tools

import ai.solace.coder.protocol.ContentBlock
import ai.solace.coder.protocol.ContentItem
import ai.solace.coder.protocol.FunctionCallOutputPayload
import ai.solace.coder.protocol.ResponseInputItem
import ai.solace.coder.protocol.ResponseItem
import ai.solace.coder.protocol.CallToolResult
import kotlinx.serialization.json.JsonNull
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class ToolCallProcessorTest {

    @Test
    fun testDefaultConfig() {
        val config = ToolCallProcessorConfig()
        assertEquals(false, config.enableParallelExecution)
        assertEquals(1, config.maxConcurrentCalls)
        assertEquals(60000L, config.defaultTimeoutMs)
    }

    @Test
    fun testCustomConfig() {
        val config = ToolCallProcessorConfig(
            enableParallelExecution = true,
            maxConcurrentCalls = 4,
            defaultTimeoutMs = 120000L
        )
        assertEquals(true, config.enableParallelExecution)
        assertEquals(4, config.maxConcurrentCalls)
        assertEquals(120000L, config.defaultTimeoutMs)
    }

    @Test
    fun testGetStatistics() {
        val processor = ToolCallProcessor(
            config = ToolCallProcessorConfig(
                enableParallelExecution = true,
                maxConcurrentCalls = 8,
                defaultTimeoutMs = 30000L
            )
        )

        val stats = processor.getStatistics()
        assertEquals(8, stats.maxConcurrentCalls)
        assertTrue(stats.parallelExecutionEnabled)
        assertEquals(30000L, stats.defaultTimeoutMs)
    }

    @Test
    fun testCreateNonToolProcessedItem() {
        val processor = ToolCallProcessor()
        val item = ResponseItem.Message(
            role = "assistant",
            content = listOf(ContentItem.OutputText(text = "Hello!"))
        )

        val processed = processor.createNonToolProcessedItem(item)
        assertEquals(item, processed.item)
        assertNull(processed.response)
    }

    @Test
    fun testCreateToolCallProcessedItem() {
        val processor = ToolCallProcessor()
        val item = ResponseItem.FunctionCall(
            name = "test_tool",
            arguments = "{}",
            call_id = "call_123"
        )
        val output = FunctionCallOutputPayload(
            content = "Tool executed successfully",
            success = true
        )

        val processed = processor.createToolCallProcessedItem(item, "call_123", output)
        assertEquals(item, processed.item)
        assertNotNull(processed.response)

        val response = processed.response as ResponseInputItem.FunctionCallOutput
        assertEquals("call_123", response.call_id)
        assertEquals("Tool executed successfully", response.output.content)
        assertEquals(true, response.output.success)
    }

    @Test
    fun testCreateErrorResponse() {
        val processor = ToolCallProcessor()
        val errorResponse = processor.createErrorResponse("call_456", "Permission denied")

        assertTrue(errorResponse is ResponseInputItem.FunctionCallOutput)
        val functionOutput = errorResponse as ResponseInputItem.FunctionCallOutput
        assertEquals("call_456", functionOutput.call_id)
        assertEquals("Permission denied", functionOutput.output.content)
        assertEquals(false, functionOutput.output.success)
    }
}

class ProcessedResponseItemTest {

    @Test
    fun testProcessedResponseItemWithResponse() {
        val item = ResponseItem.Message(
            role = "assistant",
            content = listOf(ContentItem.OutputText(text = "Result"))
        )
        val response = ResponseInputItem.FunctionCallOutput(
            call_id = "call_1",
            output = FunctionCallOutputPayload(content = "done")
        )

        val processed = ProcessedResponseItem(item = item, response = response)
        assertEquals(item, processed.item)
        assertEquals(response, processed.response)
    }

    @Test
    fun testProcessedResponseItemWithoutResponse() {
        val item = ResponseItem.Message(
            role = "assistant",
            content = listOf(ContentItem.OutputText(text = "Hello"))
        )

        val processed = ProcessedResponseItem(item = item, response = null)
        assertEquals(item, processed.item)
        assertNull(processed.response)
    }
}

class FunctionCallOutputPayloadFromCallToolResultTest {

    @Test
    fun testFromCallToolResultWithError() {
        val result = CallToolResult(
            content = listOf(ContentBlock.TextContent(text = "Error occurred")),
            is_error = true
        )

        val payload = FunctionCallOutputPayload.fromCallToolResult(result)
        // is_error=true should result in success=false
        assertEquals(false, payload.success)
    }

    @Test
    fun testFromCallToolResultIsErrorNull() {
        val result = CallToolResult(
            content = emptyList(),
            is_error = null
        )

        val payload = FunctionCallOutputPayload.fromCallToolResult(result)
        // is_error=null (not true) should result in success=true
        assertEquals(true, payload.success)
    }

    @Test
    fun testFromCallToolResultEmptyContent() {
        val result = CallToolResult(
            content = emptyList(),
            is_error = false
        )

        val payload = FunctionCallOutputPayload.fromCallToolResult(result)
        // Empty content list should still succeed
        assertEquals(true, payload.success)
        assertNull(payload.content_items) // No images
    }
}

class ProcessItemsResultTest {

    @Test
    fun testProcessItemsResultCreation() {
        val responses = listOf(
            ResponseInputItem.FunctionCallOutput(
                call_id = "call_1",
                output = FunctionCallOutputPayload(content = "output1")
            )
        )
        val items = listOf<ResponseItem>(
            ResponseItem.FunctionCallOutput(
                call_id = "call_1",
                output = FunctionCallOutputPayload(content = "output1")
            )
        )

        val result = ProcessItemsResult(
            responses = responses,
            itemsToRecord = items
        )

        assertEquals(1, result.responses.size)
        assertEquals(1, result.itemsToRecord.size)
    }
}
