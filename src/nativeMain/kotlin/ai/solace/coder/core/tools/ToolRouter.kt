package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.core.session.Session
import ai.solace.coder.core.session.TurnContext
import ai.solace.coder.protocol.models.ResponseInputItem
import ai.solace.coder.protocol.models.ResponseItem
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * Router for dispatching tool calls to appropriate handlers.
 * 
 * The ToolRouter is responsible for:
 * - Converting ResponseItems to ToolCalls
 * - Dispatching tool calls to registered handlers
 * - Managing parallel tool execution when supported
 * - Handling tool call failures and formatting responses
 */
class ToolRouter(
    private val registry: ToolRegistry
) {
    private val executionMutex = Mutex()
    
    /**
     * Get all tool specifications from the registry.
     */
    fun getToolSpecs(): List<ToolSpec> {
        val toolNames = registry.getToolNames()
        val specs = mutableListOf<ToolSpec>()

        for (name in toolNames) {
            specs.add(
                ToolSpec.Function(
                    ResponsesApiTool(
                        name = name,
                        description = "Tool: $name",
                        strict = false,
                        parameters = JsonSchema.Object(
                            properties = emptyMap(),
                            required = null,
                            additionalProperties = null
                        )
                    )
                )
            )
        }

        return specs
    }
    
    /**
     * Check if a tool supports parallel execution.
     */
    fun toolSupportsParallel(toolName: String): Boolean {
        // For now, assume all tools support parallel execution
        // In a full implementation, this would be determined by tool metadata
        return true
    }
    
    /**
     * Convert a ResponseItem to a ToolCall if it represents a tool invocation.
     */
    suspend fun buildToolCall(
        session: Session,
        item: ResponseItem
    ): CodexResult<ToolCall?> {
        return when (item) {
            is ResponseItem.FunctionCall -> {
                val payload = if (isMcpToolName(item.name)) {
                    val (server, tool) = parseMcpToolName(item.name)
                    ToolPayload.Mcp(
                        server = server,
                        tool = tool,
                        rawArguments = item.arguments
                    )
                } else if (item.name == "unified_exec") {
                    ToolPayload.UnifiedExec(arguments = item.arguments)
                } else {
                    ToolPayload.Function(arguments = item.arguments)
                }
                
                CodexResult.success(
                    ToolCall(
                        toolName = item.name,
                        callId = item.call_id,
                        payload = payload
                    )
                )
            }
            
            is ResponseItem.CustomToolCall -> {
                CodexResult.success(
                    ToolCall(
                        toolName = item.name,
                        callId = item.call_id,
                        payload = ToolPayload.Custom(input = item.input)
                    )
                )
            }
            
            is ResponseItem.LocalShellCall -> {
                val callId = item.call_id ?: item.id
                    ?: return CodexResult.failure(
                        CodexError.Fatal("Local shell call missing call_id")
                    )
                
                when (val action = item.action) {
                    is ai.solace.coder.protocol.models.LocalShellAction.Exec -> {
                        val params = ai.solace.coder.protocol.models.ShellToolCallParams(
                            command = action.command,
                            workdir = action.working_directory,
                            timeoutMs = action.timeout_ms,
                            with_escalated_permissions = null,
                            justification = null
                        )
                        
                        CodexResult.success(
                            ToolCall(
                                toolName = "local_shell",
                                callId = callId,
                                payload = ToolPayload.LocalShell(params = params)
                            )
                        )
                    }
                }
            }
            
            else -> CodexResult.success(null)
        }
    }
    
    /**
     * Dispatch a single tool call for execution.
     */
    suspend fun dispatchToolCall(
        session: Session,
        turn: TurnContext,
        call: ToolCall
    ): CodexResult<ResponseInputItem> {
        val invocation = ToolInvocation(
            session = session,
            turn = turn,
            callId = call.callId,
            toolName = call.toolName,
            payload = call.payload
        )
        
        return registry.dispatch(invocation)
    }
    
    /**
     * Dispatch multiple tool calls, executing them in parallel if supported.
     */
    suspend fun dispatchToolCalls(
        session: Session,
        turn: TurnContext,
        calls: List<ToolCall>
    ): CodexResult<List<ResponseInputItem>> {
        if (calls.isEmpty()) {
            return CodexResult.success(emptyList())
        }
        
        // Check if all tools support parallel execution
        var allSupportParallel = true
        for (call in calls) {
            if (!toolSupportsParallel(call.toolName)) {
                allSupportParallel = false
                break
            }
        }
        
        return if (allSupportParallel) {
            // Execute in parallel
            executeParallel(session, turn, calls)
        } else {
            // Execute sequentially
            executeSequential(session, turn, calls)
        }
    }
    
    /**
     * Execute tool calls in parallel.
     */
    private suspend fun executeParallel(
        session: Session,
        turn: TurnContext,
        calls: List<ToolCall>
    ): CodexResult<List<ResponseInputItem>> {
        return try {
            val scope = CoroutineScope(Dispatchers.Default)
            val deferredResults = mutableListOf<kotlinx.coroutines.Deferred<CodexResult<ResponseInputItem>>>()
            
            for (call in calls) {
                val deferred = scope.async(Dispatchers.Default) {
                    dispatchToolCall(session, turn, call)
                }
                deferredResults.add(deferred)
            }
            
            val results = deferredResults.awaitAll()
            
            // Check for any failures
            val failures = mutableListOf<CodexError>()
            val successes = mutableListOf<ResponseInputItem>()
            
            for (result in results) {
                if (result.isFailure()) {
                    failures.add(CodexError.Fatal("Tool call failed"))
                } else {
                    successes.add(result.getOrThrow())
                }
            }
            
            if (failures.isNotEmpty()) {
                val errorMessages = mutableListOf<String>()
                for (failure in failures) {
                    errorMessages.add(failure.toString())
                }
                CodexResult.failure(
                    CodexError.Fatal("Some tool calls failed: ${errorMessages.joinToString(", ")}")
                )
            } else {
                CodexResult.success(successes)
            }
        } catch (e: Exception) {
            CodexResult.failure(
                CodexError.Fatal("Parallel tool execution failed: ${e.message ?: "Unknown error"}")
            )
        }
    }
    
    /**
     * Execute tool calls sequentially.
     */
    private suspend fun executeSequential(
        session: Session,
        turn: TurnContext,
        calls: List<ToolCall>
    ): CodexResult<List<ResponseInputItem>> {
        return executionMutex.withLock {
            val results = mutableListOf<ResponseInputItem>()
            
            for (call in calls) {
                val result = dispatchToolCall(session, turn, call)
                if (result.isFailure()) {
                    return@withLock result.map { emptyList() }
                }
                results.add(result.getOrThrow())
            }
            
            CodexResult.success(results)
        }
    }
    
    /**
     * Create a failure response for a tool call.
     */
    private fun createFailureResponse(
        callId: String,
        payload: ToolPayload,
        error: String
    ): ResponseInputItem {
        return when (payload) {
            is ToolPayload.Custom -> {
                ResponseInputItem.CustomToolCallOutput(
                    call_id = callId,
                    output = error
                )
            }
            else -> {
                ResponseInputItem.FunctionCallOutput(
                    call_id = callId,
                    output = ai.solace.coder.protocol.models.FunctionCallOutputPayload(
                        content = error,
                        success = false
                    )
                )
            }
        }
    }
    
    /**
     * Check if a tool name follows the MCP naming convention.
     */
    private fun isMcpToolName(name: String): Boolean {
        return name.startsWith("mcp__")
    }
    
    /**
     * Parse an MCP tool name to extract server and tool names.
     */
    private fun parseMcpToolName(name: String): Pair<String, String> {
        // Expected format: mcp__server__tool
        val parts = name.split("__")
        return if (parts.size >= 3 && parts[0] == "mcp") {
            val server = parts[1]
            val toolParts = parts.subList(2, parts.size)
            val tool = toolParts.joinToString("__")
            Pair(server, tool)
        } else {
            Pair("", name)
        }
    }
    
    /**
     * Dispatch a tool call with turn diff tracker (for ToolCallRuntime compatibility).
     */
    suspend fun dispatchToolCall(
        session: Session,
        turn: TurnContext,
        tracker: SharedTurnDiffTracker,
        call: ToolCall
    ): CodexResult<ResponseInputItem> {
        // Tracker can be used for diff tracking in the future
        return dispatchToolCall(session, turn, call)
    }

    companion object {
        /**
         * Create a ToolRouter with the given registry.
         */
        fun create(registry: ToolRegistry): ToolRouter {
            return ToolRouter(registry)
        }
    }
}