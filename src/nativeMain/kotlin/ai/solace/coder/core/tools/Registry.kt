// port-lint: source core/src/tools/registry.rs
package ai.solace.coder.core.tools

import ai.solace.coder.client.common.tools.ToolSpec
import ai.solace.coder.core.function_tool.FunctionCallError
import ai.solace.coder.protocol.ResponseInputItem
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlin.time.Duration

enum class ToolKind {
    Function,
    Mcp
}

interface ToolHandler {
    fun kind(): ToolKind

    fun matchesKind(payload: ToolPayload): Boolean {
        return when (kind()) {
            ToolKind.Function -> payload is ToolPayload.Function
            ToolKind.Mcp -> payload is ToolPayload.Mcp
        }
    }

    fun isMutating(invocation: ToolInvocation): Boolean {
        return false
    }

    suspend fun handle(invocation: ToolInvocation): Result<ToolOutput>
}

class ToolRegistry(
    private val handlers: Map<String, ToolHandler>
) {
    fun handler(name: String): ToolHandler? {
        return handlers[name]
    }

    suspend fun dispatch(
        invocation: ToolInvocation
    ): Result<ResponseInputItem> {
        val toolName = invocation.toolName
        val callIdOwned = invocation.callId
        val otel = invocation.turn.client.getOtelEventManager()
        val payloadForResponse = invocation.payload
        val logPayload = payloadForResponse.logPayload()

        val handler = handler(toolName) ?: run {
            val message = unsupportedToolCallMessage(invocation.payload, toolName)
            otel.toolResult(
                toolName,
                callIdOwned,
                logPayload,
                Duration.ZERO,
                false,
                message
            )
            return Result.failure(FunctionCallError.RespondToModel(message))
        }

        if (!handler.matchesKind(invocation.payload)) {
            val message = "tool $toolName invoked with incompatible payload"
            otel.toolResult(
                toolName,
                callIdOwned,
                logPayload,
                Duration.ZERO,
                false,
                message
            )
            return Result.failure(FunctionCallError.Fatal(message))
        }

        val outputCell = Mutex()
        var outputValue: ToolOutput? = null

        // Note: Kotlin doesn't have exact equivalent of Rust's closure-based logging wrapper 
        // in the same way, but we can adapt.
        // Assuming otel.logToolResult takes a suspend block.
        
        val result = otel.logToolResult(
            toolName,
            callIdOwned,
            logPayload
        ) {
            if (handler.isMutating(invocation)) {
                // tracing::trace!("waiting for tool gate")
                invocation.turn.toolCallGate.waitReady()
                // tracing::trace!("tool gate released")
            }
            
            val handleResult = handler.handle(invocation)
            handleResult.fold(
                onSuccess = { output ->
                    val preview = output.logPreview()
                    val success = output.successForLogging()
                    outputCell.withLock {
                        outputValue = output
                    }
                    Pair(preview, success)
                },
                onFailure = { err ->
                    throw err // Or handle error return
                }
            )
        }

        return try {
            // If result was successful (meaning the block executed without throwing)
            // we retrieve the output.
            val output = outputCell.withLock { outputValue } 
                ?: throw FunctionCallError.Fatal("tool produced no output")
            
            Result.success(output.intoResponse(callIdOwned, payloadForResponse))
        } catch (e: Exception) {
            Result.failure(if (e is FunctionCallError) e else FunctionCallError.Fatal(e.message ?: "Unknown error"))
        }
    }
}

data class ConfiguredToolSpec(
    val spec: ToolSpec,
    val supportsParallelToolCalls: Boolean
)

class ToolRegistryBuilder {
    private val handlers = mutableMapOf<String, ToolHandler>()
    private val specs = mutableListOf<ConfiguredToolSpec>()

    fun pushSpec(spec: ToolSpec) {
        pushSpecWithParallelSupport(spec, false)
    }

    fun pushSpecWithParallelSupport(
        spec: ToolSpec,
        supportsParallelToolCalls: Boolean
    ) {
        specs.add(ConfiguredToolSpec(spec, supportsParallelToolCalls))
    }

    fun registerHandler(name: String, handler: ToolHandler) {
        if (handlers.put(name, handler) != null) {
            // warn!("overwriting handler for tool {name}");
            println("overwriting handler for tool $name")
        }
    }

    fun build(): Pair<List<ConfiguredToolSpec>, ToolRegistry> {
        val registry = ToolRegistry(handlers.toMap())
        return Pair(specs.toList(), registry)
    }
}

private fun unsupportedToolCallMessage(payload: ToolPayload, toolName: String): String {
    return when (payload) {
        is ToolPayload.Custom -> "unsupported custom tool call: $toolName"
        else -> "unsupported call: $toolName"
    }
}
