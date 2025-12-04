// port-lint: source core/src/tools/router.rs
package ai.solace.coder.core.tools

import ai.solace.coder.core.FunctionCallError
import ai.solace.coder.core.session.Session
import ai.solace.coder.core.session.ToolSpec
import ai.solace.coder.core.session.TurnContext
import ai.solace.coder.protocol.FunctionCallOutputPayload
import ai.solace.coder.protocol.LocalShellAction
import ai.solace.coder.protocol.ResponseInputItem
import ai.solace.coder.protocol.ResponseItem
import ai.solace.coder.protocol.ShellToolCallParams

data class ToolCall(
    val toolName: String,
    val callId: String,
    val payload: ToolPayload
)

class ToolRouter(
    private val registry: ToolRegistry,
    private val specs: List<ConfiguredToolSpec>
) {
    companion object {
        fun fromConfig(
            config: ToolsConfig,
            mcpTools: Map<String, McpTool>?
        ): ToolRouter {
            val builder = buildSpecs(config, mcpTools)
            val (specs, registry) = builder.build()
            return ToolRouter(registry, specs)
        }
    }

    fun specs(): List<ToolSpec> {
        return specs.map { it.spec }
    }

    fun toolSupportsParallel(toolName: String): Boolean {
        return specs
            .filter { it.supportsParallelToolCalls }
            .any { it.spec.name == toolName }
    }

    suspend fun buildToolCall(
        session: Session,
        item: ResponseItem
    ): Result<ToolCall?> {
        return when (item) {
            is ResponseItem.FunctionCall -> {
                val name = item.name
                val arguments = item.arguments
                val callId = item.callId

                val mcpTool = session.parseMcpToolName(name)
                if (mcpTool != null) {
                    val (server, tool) = mcpTool
                    Result.success(ToolCall(
                        toolName = name,
                        callId = callId,
                        payload = ToolPayload.Mcp(
                            server = server,
                            tool = tool,
                            rawArguments = arguments
                        )
                    ))
                } else {
                    val payload = if (name == "unified_exec") {
                        ToolPayload.UnifiedExec(arguments)
                    } else {
                        ToolPayload.Function(arguments)
                    }
                    Result.success(ToolCall(
                        toolName = name,
                        callId = callId,
                        payload = payload
                    ))
                }
            }
            is ResponseItem.CustomToolCall -> {
                Result.success(ToolCall(
                    toolName = item.name,
                    callId = item.callId,
                    payload = ToolPayload.Custom(item.input)
                ))
            }
            is ResponseItem.LocalShellCall -> {
                val callId = item.callId ?: item.id ?: return Result.failure(FunctionCallError.MissingLocalShellCallId)
                
                when (val action = item.action) {
                    is LocalShellAction.Exec -> {
                        val params = ShellToolCallParams(
                            command = action.command,
                            workdir = action.workingDirectory,
                            timeoutMs = action.timeoutMs,
                            withEscalatedPermissions = null,
                            justification = null
                        )
                        Result.success(ToolCall(
                            toolName = "local_shell",
                            callId = callId,
                            payload = ToolPayload.LocalShell(params)
                        ))
                    }
                }
            }
            else -> Result.success(null)
        }
    }

    suspend fun dispatchToolCall(
        session: Session,
        turn: TurnContext,
        tracker: SharedTurnDiffTracker,
        call: ToolCall
    ): Result<ResponseInputItem> {
        val toolName = call.toolName
        val callId = call.callId
        val payload = call.payload
        val payloadOutputsCustom = payload is ToolPayload.Custom
        val failureCallId = callId

        val invocation = ToolInvocation(
            session = session,
            turn = turn,
            tracker = tracker,
            callId = callId,
            toolName = toolName,
            payload = payload
        )

        return try {
            val result = registry.dispatch(invocation)
            result.fold(
                onSuccess = { response -> Result.success(response) },
                onFailure = { e ->
                    if (e is FunctionCallError.Fatal) {
                        Result.failure(e)
                    } else {
                        // Recoverable error, return failure response
                        val err = if (e is FunctionCallError) e else FunctionCallError.Fatal(e.message ?: "Unknown error")
                        Result.success(failureResponse(failureCallId, payloadOutputsCustom, err))
                    }
                }
            )
        } catch (e: Exception) {
            // Should not happen if dispatch catches everything, but just in case
            val err = if (e is FunctionCallError) e else FunctionCallError.Fatal(e.message ?: "Unknown error")
             Result.success(failureResponse(failureCallId, payloadOutputsCustom, err))
        }
    }

    private fun failureResponse(
        callId: String,
        payloadOutputsCustom: Boolean,
        err: FunctionCallError
    ): ResponseInputItem {
        val message = err.toString()
        return if (payloadOutputsCustom) {
            ResponseInputItem.CustomToolCallOutput(
                callId = callId,
                output = message
            )
        } else {
            ResponseInputItem.FunctionCallOutput(
                callId = callId,
                output = FunctionCallOutputPayload(
                    content = message,
                    success = false
                )
            )
        }
    }
}
