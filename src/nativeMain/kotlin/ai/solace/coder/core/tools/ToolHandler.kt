package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.core.session.Session
import ai.solace.coder.core.session.TurnContext
import ai.solace.coder.protocol.models.ResponseInputItem
import ai.solace.coder.utils.concurrent.CancellationToken
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.job
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlin.time.TimeSource
import kotlin.time.measureTime

/**
 * Enumeration of tool kinds for routing purposes.
 */
enum class ToolKind {
    Function,
    Mcp,
    Custom
}

/**
 * Payload types for different tool invocations.
 */
sealed class ToolPayload {
    data class Function(
        val arguments: String
    ) : ToolPayload()
    
    data class Custom(
        val input: String
    ) : ToolPayload()
    
    data class LocalShell(
        val params: ai.solace.coder.protocol.models.ShellToolCallParams
    ) : ToolPayload()
    
    data class UnifiedExec(
        val arguments: String
    ) : ToolPayload()
    
    data class Mcp(
        val server: String,
        val tool: String,
        val rawArguments: String
    ) : ToolPayload()
    
    /**
     * Get a log-friendly representation of the payload.
     */
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

/**
 * Output types for tool executions.
 */
sealed class ToolOutput {
    data class Function(
        val content: String,
        val contentItems: List<ai.solace.coder.protocol.models.FunctionCallOutputContentItem>? = null,
        val success: Boolean? = null
    ) : ToolOutput()

    data class Mcp(
        val result: ai.solace.coder.protocol.models.Result<ai.solace.coder.protocol.models.CallToolResult, String>
    ) : ToolOutput()

    /**
     * Image attachment output for view_image tool.
     * The path is injected into the session as a local image.
     */
    data class ImageAttachment(
        val path: String,
        val message: String = "attached local image path"
    ) : ToolOutput()
    
    /**
     * Convert to a response input item.
     */
    fun toResponseInputItem(callId: String, payload: ToolPayload): ResponseInputItem {
        return when (this) {
            is Function -> {
                if (payload is ToolPayload.Custom) {
                    ResponseInputItem.CustomToolCallOutput(
                        call_id = callId,
                        output = content
                    )
                } else {
                    ResponseInputItem.FunctionCallOutput(
                        call_id = callId,
                        output = ai.solace.coder.protocol.models.FunctionCallOutputPayload(
                            content = content,
                            content_items = contentItems,
                            success = success
                        )
                    )
                }
            }
            is Mcp -> {
                ResponseInputItem.McpToolCallOutput(
                    call_id = callId,
                    result = result
                )
            }
            is ImageAttachment -> {
                // Image attachments are handled specially - they inject the image
                // into the conversation via UserInput::LocalImage
                ResponseInputItem.FunctionCallOutput(
                    call_id = callId,
                    output = ai.solace.coder.protocol.models.FunctionCallOutputPayload(
                        content = message,
                        content_items = null,
                        success = true
                    )
                )
            }
        }
    }
    
    /**
     * Get a preview for logging purposes.
     */
    fun logPreview(): String {
        return when (this) {
            is Function -> {
                if (content.length > 200) {
                    content.substring(0, 200) + "..."
                } else {
                    content
                }
            }
            is Mcp -> result.toString()
            is ImageAttachment -> "Image: $path"
        }
    }

    /**
     * Get success status for logging.
     */
    fun successForLogging(): Boolean {
        return when (this) {
            is Function -> success ?: true
            is Mcp -> result.isSuccess
            is ImageAttachment -> true
        }
    }
}

/**
 * Context for a tool invocation.
 */
data class ToolInvocation(
    val session: Session,
    val turn: TurnContext,
    val callId: String,
    val toolName: String,
    val payload: ToolPayload,
    val cancellationToken: CancellationToken = CancellationToken()
)

/**
 * Represents a tool call to be executed.
 */
data class ToolCall(
    val callId: String,
    val toolName: String,
    val payload: ToolPayload
)

/**
 * Base interface for all tool handlers.
 */
interface ToolHandler {
    /**
     * The kind of tool this handler supports.
     */
    val kind: ToolKind
    
    /**
     * Check if this handler matches the given payload type.
     */
    fun matchesKind(payload: ToolPayload): Boolean {
        return when (kind) {
            ToolKind.Function -> payload is ToolPayload.Function || payload is ToolPayload.LocalShell
            ToolKind.Mcp -> payload is ToolPayload.Mcp
            ToolKind.Custom -> payload is ToolPayload.Custom
        }
    }
    
    /**
     * Check if this tool is mutating (requires exclusive access).
     */
    fun isMutating(invocation: ToolInvocation): Boolean {
        return false // Default to non-mutating
    }
    
    /**
     * Handle a tool invocation.
     */
    suspend fun handle(invocation: ToolInvocation): CodexResult<ToolOutput>
    
    /**
     * Get the timeout for this tool execution in milliseconds.
     */
    fun getTimeoutMs(): Long = 60000L // 60 seconds default
    
    /**
     * Execute the tool with timeout and cancellation support.
     */
    suspend fun executeWithTimeout(
        invocation: ToolInvocation,
        scope: CoroutineScope
    ): CodexResult<ToolOutput> {
        return try {
            // Check for cancellation before execution
            if (invocation.cancellationToken.isCancelled()) {
                return CodexResult.failure(
                    CodexError.Fatal("Tool execution was cancelled")
                )
            }
            
            var result: CodexResult<ToolOutput>? = null
            var completed = false
            
            val job = scope.launch(Dispatchers.Default) {
                try {
                    result = handle(invocation)
                    completed = true
                } catch (e: Exception) {
                    result = CodexResult.failure(
                        CodexError.Fatal("Tool execution failed: ${e.message ?: "Unknown error"}")
                    )
                }
            }
            
            // Wait for completion
            job.join()

            result ?: CodexResult.failure(
                CodexError.Fatal("Tool execution failed to produce result")
            )
            
        } catch (e: Exception) {
            CodexResult.failure(
                CodexError.Fatal("Tool execution failed: ${e.message ?: "Unknown error"}")
            )
        }
    }
}

/**
 * Registry for managing tool handlers.
 */
class ToolRegistry {
    private val handlers = mutableMapOf<String, ToolHandler>()
    
    /**
     * Register a tool handler.
     */
    fun register(name: String, handler: ToolHandler) {
        handlers[name] = handler
    }
    
    /**
     * Get a handler by name.
     */
    fun getHandler(name: String): ToolHandler? {
        return handlers[name]
    }
    
    /**
     * Get all registered tool names.
     */
    fun getToolNames(): Set<String> {
        return handlers.keys.toSet()
    }
    
    /**
     * Check if a tool is registered.
     */
    fun hasTool(name: String): Boolean {
        return handlers.containsKey(name)
    }
    
    /**
     * Dispatch a tool invocation to the appropriate handler.
     */
    suspend fun dispatch(invocation: ToolInvocation): CodexResult<ResponseInputItem> {
        val handler = getHandler(invocation.toolName)
            ?: return CodexResult.failure(
                CodexError.Fatal("Unknown tool: ${invocation.toolName}")
            )
        
        if (!handler.matchesKind(invocation.payload)) {
            return CodexResult.failure(
                CodexError.Fatal("Tool ${invocation.toolName} received incompatible payload")
            )
        }
        
        return try {
            val output = handler.executeWithTimeout(
                invocation,
                CoroutineScope(Dispatchers.Default)
            )
            output.map { toolOutput ->
                toolOutput.toResponseInputItem(invocation.callId, invocation.payload)
            }
        } catch (e: Exception) {
            CodexResult.failure(
                CodexError.Fatal("Tool execution failed: ${e.message ?: "Unknown error"}")
            )
        }
    }
}