// port-lint: source core/src/tools/parallel.rs
package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.session.Session
import ai.solace.coder.core.session.TurnContext
import ai.solace.coder.protocol.FunctionCallOutputPayload
import ai.solace.coder.protocol.ResponseInputItem
import kotlinx.coroutines.Job
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.selects.select
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlin.time.TimeSource

/**
 * A ReadWriteMutex that allows multiple readers or a single writer.
 * Matches tokio::sync::RwLock semantics used in Rust.
 */
class ReadWriteMutex {
    private val writeMutex = Mutex()
    private val readMutex = Mutex()
    private var readers = 0

    suspend fun <T> read(action: suspend () -> T): T {
        readMutex.lock()
        try {
            if (readers == 0) {
                writeMutex.lock()
            }
            readers++
        } finally {
            readMutex.unlock()
        }

        try {
            return action()
        } finally {
            readMutex.lock()
            try {
                readers--
                if (readers == 0) {
                    writeMutex.unlock()
                }
            } finally {
                readMutex.unlock()
            }
        }
    }

    suspend fun <T> write(action: suspend () -> T): T {
        writeMutex.withLock {
            return action()
        }
    }
}

class ToolCallRuntime(
    private val router: ToolRouter,
    private val session: Session,
    private val turnContext: TurnContext,
    private val tracker: SharedTurnDiffTracker
) {
    private val parallelExecution = ReadWriteMutex()

    suspend fun handleToolCall(
        call: ToolCall,
        cancellationJob: Job?
    ): Result<ResponseInputItem> {
        val supportsParallel = router.toolSupportsParallel(call.toolName)
        val started = TimeSource.Monotonic.markNow()

        return try {
            coroutineScope {
                var result: Result<ResponseInputItem>? = null
                
                val job = launch {
                    if (supportsParallel) {
                        parallelExecution.read {
                            result = router.dispatchToolCall(session, turnContext, tracker, call.copy())
                        }
                    } else {
                        parallelExecution.write {
                            result = router.dispatchToolCall(session, turnContext, tracker, call.copy())
                        }
                    }
                }

                if (cancellationJob != null) {
                    select {
                        cancellationJob.onJoin {
                            job.cancel()
                            val secs = started.elapsedNow().inWholeMilliseconds / 1000.0f
                            result = Result.success(abortedResponse(call, secs.coerceAtLeast(0.1f)))
                        }
                        job.join()
                    }
                } else {
                    job.join()
                }
                
                result ?: Result.failure(CodexError.Fatal("Tool execution failed to produce result"))
            }
        } catch (e: Exception) {
             Result.failure(CodexError.Fatal("tool task failed to receive: ${e.message}"))
        }
    }

    private fun abortedResponse(call: ToolCall, secs: Float): ResponseInputItem {
        return when (val payload = call.payload) {
            is ToolPayload.Custom -> ResponseInputItem.CustomToolCallOutput(
                callId = call.callId,
                output = abortMessage(call, secs)
            )
            is ToolPayload.Mcp -> ResponseInputItem.McpToolCallOutput(
                callId = call.callId,
                result = ai.solace.coder.protocol.Result(
                    value = null,
                    error = abortMessage(call, secs)
                )
            )
            else -> ResponseInputItem.FunctionCallOutput(
                callId = call.callId,
                output = FunctionCallOutputPayload(
                    content = abortMessage(call, secs),
                    success = false // Default
                )
            )
        }
    }

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
