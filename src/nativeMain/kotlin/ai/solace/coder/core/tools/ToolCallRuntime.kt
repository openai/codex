package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.core.session.Session
import ai.solace.coder.core.session.TurnContext
import ai.solace.coder.protocol.models.FunctionCallOutputPayload
import ai.solace.coder.protocol.models.ResponseInputItem
import kotlinx.coroutines.Job
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.selects.select
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlin.time.TimeSource

/**
 * Runtime for handling tool calls with parallel execution support.
 *
 * Uses a Mutex to coordinate parallel vs sequential execution:
 * - Tools that support parallel execution can run concurrently
 * - Tools that don't support parallel execution acquire exclusive access
 *
 * Ported from Rust codex-rs/core/src/tools/parallel.rs
 */
class ToolCallRuntime(
    private val router: ToolRouter,
    private val session: Session,
    private val turnContext: TurnContext,
    private val tracker: SharedTurnDiffTracker
) {
    /**
     * Mutex for coordinating parallel execution.
     * In Rust, this uses RwLock with read() for parallel and write() for exclusive.
     * Kotlin's Mutex is always exclusive, so we track parallel execution count separately.
     */
    private val executionMutex = Mutex()
    private var parallelExecutionCount = 0
    private val parallelCountMutex = Mutex()

    /**
     * Handle a tool call with cancellation support.
     *
     * @param call The tool call to execute
     * @param cancellationJob Job that can be cancelled to abort the tool call
     * @return Result containing the response item or an error
     */
    suspend fun handleToolCall(
        call: ToolCall,
        cancellationJob: Job?
    ): CodexResult<ResponseInputItem> {
        val supportsParallel = router.toolSupportsParallel(call.toolName)
        val started = TimeSource.Monotonic.markNow()

        return try {
            coroutineScope {
                // Use select to handle cancellation
                if (cancellationJob != null) {
                    select {
                        cancellationJob.onJoin {
                            // Cancelled
                            val secs = started.elapsedNow().inWholeMilliseconds / 1000.0f
                            CodexResult.success(abortedResponse(call, secs.coerceAtLeast(0.1f)))
                        }
                    }
                } else {
                    // No cancellation, execute directly
                    executeWithLock(call, supportsParallel)
                }
            }
        } catch (e: Exception) {
            CodexResult.failure(
                CodexError.Fatal("tool task failed: ${e.message}")
            )
        }
    }

    /**
     * Execute tool call with appropriate locking.
     */
    private suspend fun executeWithLock(
        call: ToolCall,
        supportsParallel: Boolean
    ): CodexResult<ResponseInputItem> {
        return if (supportsParallel) {
            // For parallel execution, track count but don't block other parallel tools
            parallelCountMutex.withLock {
                parallelExecutionCount++
            }
            try {
                dispatchToolCall(call)
            } finally {
                parallelCountMutex.withLock {
                    parallelExecutionCount--
                }
            }
        } else {
            // For sequential execution, acquire exclusive lock
            executionMutex.withLock {
                // Wait for parallel executions to complete
                while (true) {
                    val count = parallelCountMutex.withLock { parallelExecutionCount }
                    if (count == 0) break
                    // Small delay to avoid busy waiting
                    kotlinx.coroutines.delay(1)
                }
                dispatchToolCall(call)
            }
        }
    }

    /**
     * Dispatch the tool call to the router.
     */
    private suspend fun dispatchToolCall(call: ToolCall): CodexResult<ResponseInputItem> {
        return router.dispatchToolCall(session, turnContext, tracker, call)
    }

    /**
     * Create an aborted response for a tool call.
     */
    private fun abortedResponse(call: ToolCall, secs: Float): ResponseInputItem {
        return when (call.payload) {
            is ToolPayload.Custom -> ResponseInputItem.CustomToolCallOutput(
                call_id = call.callId,
                output = abortMessage(call, secs)
            )
            is ToolPayload.Mcp -> ResponseInputItem.McpToolCallOutput(
                call_id = call.callId,
                result = ai.solace.coder.protocol.models.Result(
                    value = null,
                    error = abortMessage(call, secs)
                )
            )
            else -> ResponseInputItem.FunctionCallOutput(
                call_id = call.callId,
                output = FunctionCallOutputPayload(
                    content = abortMessage(call, secs)
                )
            )
        }
    }

    /**
     * Create an abort message for a tool call.
     */
    private fun abortMessage(call: ToolCall, secs: Float): String {
        val formattedSecs = ((secs * 10).toInt() / 10.0).toString()
        return when (call.toolName) {
            "shell", "container.exec", "local_shell", "shell_command", "unified_exec" -> {
                "Wall time: $formattedSecs seconds\naborted by user"
            }
            else -> "aborted by user after ${formattedSecs}s"
        }
    }
}

/**
 * Shared tracker for turn diffs.
 * Tracks file modifications and other changes during a turn.
 */
class SharedTurnDiffTracker {
    private val mutex = Mutex()
    private val diffs = mutableListOf<TurnDiff>()

    suspend fun addDiff(diff: TurnDiff) {
        mutex.withLock {
            diffs.add(diff)
        }
    }

    suspend fun getDiffs(): List<TurnDiff> {
        return mutex.withLock {
            diffs.toList()
        }
    }

    suspend fun clear() {
        mutex.withLock {
            diffs.clear()
        }
    }
}

/**
 * Represents a diff/change made during a turn.
 */
sealed class TurnDiff {
    data class FileModified(val path: String, val content: String?) : TurnDiff()
    data class FileCreated(val path: String) : TurnDiff()
    data class FileDeleted(val path: String) : TurnDiff()
}
