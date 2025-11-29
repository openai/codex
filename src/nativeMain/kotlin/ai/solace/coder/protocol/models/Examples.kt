package ai.solace.coder.protocol.models

import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/**
 * Example usage of protocol models for documentation and validation purposes.
 */
object ProtocolModelsExamples {
    private val json = Json {
        prettyPrint = true
        ignoreUnknownKeys = true
    }

    /**
     * Example: Creating and serializing a simple message.
     */
    fun createUserMessage(): String {
        val message = ResponseInputItem.Message(
            role = "user",
            content = listOf(
                ContentItem.InputText(text = "Hello, how can I help?")
            )
        )
        return json.encodeToString(message)
    }

    /**
     * Example: Creating a function call output with plain string.
     */
    fun createSimpleFunctionCallOutput(): String {
        val output = ResponseInputItem.FunctionCallOutput(
            call_id = "call_123",
            output = FunctionCallOutputPayload(
                content = "Command executed successfully",
                content_items = null,
                success = null
            )
        )
        return json.encodeToString(output)
    }

    /**
     * Example: Creating a function call output with structured content.
     */
    fun createStructuredFunctionCallOutput(): String {
        val output = ResponseInputItem.FunctionCallOutput(
            call_id = "call_456",
            output = FunctionCallOutputPayload(
                content = """[{"type":"input_text","text":"Result"}]""",
                content_items = listOf(
                    FunctionCallOutputContentItem.InputText(text = "Result")
                ),
                success = null
            )
        )
        return json.encodeToString(output)
    }

    /**
     * Example: Creating a local shell call.
     */
    fun createLocalShellCall(): String {
        val shellCall = ResponseItem.LocalShellCall(
            id = "shell_1",
            call_id = "call_789",
            status = LocalShellStatus.Completed,
            action = LocalShellAction.Exec(
                command = listOf("ls", "-la"),
                timeout_ms = 5000,
                working_directory = "/tmp",
                env = mapOf("PATH" to "/usr/bin"),
                user = null
            )
        )
        return json.encodeToString(shellCall)
    }

    /**
     * Example: Creating a web search action.
     */
    fun createWebSearchCall(): String {
        val webSearch = ResponseItem.WebSearchCall(
            id = "ws_1",
            status = "completed",
            action = WebSearchAction.Search(query = "weather in Seattle")
        )
        return json.encodeToString(webSearch)
    }

    /**
     * Example: Creating a reasoning item.
     */
    fun createReasoningItem(): String {
        val reasoning = ResponseItem.Reasoning(
            id = "reason_1",
            summary = listOf(
                ReasoningItemReasoningSummary.SummaryText(text = "Analyzing the request")
            ),
            content = listOf(
                ReasoningItemContent.ReasoningText(text = "Step 1: Parse input"),
                ReasoningItemContent.Text(text = "Step 2: Process data")
            ),
            encrypted_content = null
        )
        return json.encodeToString(reasoning)
    }

    /**
     * Example: Parsing shell tool call parameters.
     */
    fun parseShellToolCallParams(jsonString: String): ShellToolCallParams {
        return json.decodeFromString(jsonString)
    }

    /**
     * Example: Creating shell tool call parameters.
     */
    fun createShellToolCallParams(): String {
        val params = ShellToolCallParams(
            command = listOf("npm", "install"),
            workdir = "/app",
            timeoutMs = 30000,
            with_escalated_permissions = false,
            justification = "Install dependencies"
        )
        return json.encodeToString(params)
    }
}