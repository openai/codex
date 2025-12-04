// port-lint: source core/src/tools/registry.rs
package ai.solace.coder.core.tools

import ai.solace.coder.core.FunctionCallError
import ai.solace.coder.core.session.ToolSpec
import ai.solace.coder.protocol.ResponseInputItem

/**
 * The kind of tool handler.
 */
enum class ToolKind {
    Function,
    Mcp
}

/**
 * A handler for a specific tool type.
 *
 * Implements the async_trait pattern from Rust.
 */
interface ToolHandler {
    /**
     * The kind of tool this handler supports.
     */
    val kind: ToolKind

    /**
     * Check if this handler matches the given payload kind.
     */
    fun matchesKind(payload: ToolPayload): Boolean {
        return when (kind) {
            ToolKind.Function -> payload is ToolPayload.Function
            ToolKind.Mcp -> payload is ToolPayload.Mcp
        }
    }

    /**
     * Whether this tool invocation is mutating (requires waiting for tool gate).
     */
    fun isMutating(invocation: ToolInvocation): Boolean = false

    /**
     * Handle the tool invocation and return the result.
     */
    suspend fun handle(invocation: ToolInvocation): Result<ToolOutput>
}

/**
 * Registry of tool handlers keyed by tool name.
 */
class ToolRegistry(
    private val handlers: Map<String, ToolHandler>
) {
    /**
     * Get a handler by tool name.
     */
    fun handler(name: String): ToolHandler? = handlers[name]

    /**
     * Dispatch a tool invocation to the appropriate handler.
     *
     * This follows the Rust implementation pattern from registry.rs dispatch().
     */
    suspend fun dispatch(
        invocation: ToolInvocation
    ): Result<ResponseInputItem> {
        val toolName = invocation.toolName
        val callIdOwned = invocation.callId
        val payloadForResponse = invocation.payload
        // Note: OTEL logging is omitted here as TurnContext.client is not yet ported.
        // In a full implementation, this would be:
        // val otel = invocation.turn.client.getOtelEventManager()

        val handler = handler(toolName)
            ?: return Result.failure(
                FunctionCallError.RespondToModel(
                    unsupportedToolCallMessage(invocation.payload, toolName)
                )
            )

        if (!handler.matchesKind(invocation.payload)) {
            val message = "tool $toolName invoked with incompatible payload"
            return Result.failure(FunctionCallError.Fatal(message))
        }

        // Wait for tool gate if this is a mutating operation
        if (handler.isMutating(invocation)) {
            // tracing::trace!("waiting for tool gate")
            invocation.turn.toolCallGate?.waitReady()
            // tracing::trace!("tool gate released")
        }

        // Execute the handler
        val handleResult = handler.handle(invocation)

        return handleResult.fold(
            onSuccess = { output ->
                Result.success(output.intoResponse(callIdOwned, payloadForResponse))
            },
            onFailure = { err ->
                if (err is FunctionCallError) {
                    Result.failure(err)
                } else {
                    Result.failure(FunctionCallError.Fatal(err.message ?: "Unknown error"))
                }
            }
        )
    }
}

/**
 * A tool specification with parallel support configuration.
 */
data class ConfiguredToolSpec(
    val spec: ToolSpec,
    val supportsParallelToolCalls: Boolean
) {
    companion object {
        /**
         * Create a new ConfiguredToolSpec.
         */
        fun new(spec: ToolSpec, supportsParallelToolCalls: Boolean): ConfiguredToolSpec =
            ConfiguredToolSpec(spec, supportsParallelToolCalls)
    }
}

/**
 * Builder for constructing a ToolRegistry with tool specs.
 */
class ToolRegistryBuilder {
    private val handlers = mutableMapOf<String, ToolHandler>()
    private val specs = mutableListOf<ConfiguredToolSpec>()

    /**
     * Add a tool spec without parallel support.
     */
    fun pushSpec(spec: ToolSpec) {
        pushSpecWithParallelSupport(spec, false)
    }

    /**
     * Add a tool spec with configurable parallel support.
     */
    fun pushSpecWithParallelSupport(
        spec: ToolSpec,
        supportsParallelToolCalls: Boolean
    ) {
        specs.add(ConfiguredToolSpec.new(spec, supportsParallelToolCalls))
    }

    /**
     * Register a handler for a tool name.
     */
    fun registerHandler(name: String, handler: ToolHandler) {
        if (handlers.put(name, handler) != null) {
            // warn!("overwriting handler for tool {name}")
            println("WARN: overwriting handler for tool $name")
        }
    }

    /**
     * Build the registry and return the specs and registry.
     */
    fun build(): Pair<List<ConfiguredToolSpec>, ToolRegistry> {
        val registry = ToolRegistry(handlers.toMap())
        return Pair(specs.toList(), registry)
    }

    companion object {
        /**
         * Create a new empty builder.
         */
        fun new(): ToolRegistryBuilder = ToolRegistryBuilder()
    }
}

private fun unsupportedToolCallMessage(payload: ToolPayload, toolName: String): String {
    return when (payload) {
        is ToolPayload.Custom -> "unsupported custom tool call: $toolName"
        else -> "unsupported call: $toolName"
    }
}
