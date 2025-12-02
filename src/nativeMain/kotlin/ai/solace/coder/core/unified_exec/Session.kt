// port-lint: source core/src/unified_exec/session.rs
package ai.solace.coder.core.unified_exec

import ai.solace.coder.core.ExecToolCallOutput
import ai.solace.coder.core.SandboxType
import ai.solace.coder.core.StreamOutput
import ai.solace.coder.core.context.TruncationPolicy
import ai.solace.coder.core.context.formattedTruncateText
import ai.solace.coder.utils.pty.ExecCommandSession
import ai.solace.coder.utils.pty.SpawnedPty
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.ReceiveChannel
import kotlinx.coroutines.channels.SendChannel
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.launch
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds
import ai.solace.coder.core.isLikelySandboxDenied
import ai.solace.coder.core.unified_exec.UnifiedExecError

// Constants from mod.rs (need to be defined somewhere, assuming in UnifiedExec.kt or here)
const val UNIFIED_EXEC_OUTPUT_MAX_BYTES = 1024 * 1024 // Placeholder value
const val UNIFIED_EXEC_OUTPUT_MAX_TOKENS = 1000 // Placeholder value

class OutputBufferState {
    val chunks = ArrayDeque<ByteArray>()
    var totalBytes: Int = 0

    fun pushChunk(chunk: ByteArray) {
        totalBytes += chunk.size
        chunks.addLast(chunk)

        var excess = totalBytes - UNIFIED_EXEC_OUTPUT_MAX_BYTES
        while (excess > 0) {
            val front = chunks.firstOrNull() ?: break
            if (excess >= front.size) {
                excess -= front.size
                totalBytes -= front.size
                chunks.removeFirst()
            } else {
                // Partial drain
                // Kotlin ArrayDeque doesn't support easy partial drain of ByteArray.
                // We'll replace the first chunk with a sliced one.
                val newFront = front.copyOfRange(excess, front.size)
                chunks.removeFirst()
                chunks.addFirst(newFront)
                totalBytes -= excess
                break
            }
        }
    }

    fun drain(): List<ByteArray> {
        val drained = chunks.toList()
        chunks.clear()
        totalBytes = 0
        return drained
    }

    fun snapshot(): List<ByteArray> {
        return chunks.toList()
    }
}

typealias OutputBuffer = Mutex // Wrapping OutputBufferState

data class OutputHandles(
    val outputBuffer: Mutex, // Guarding OutputBufferState
    val outputBufferState: OutputBufferState, // Direct access reference for lock holder? No, need to pass the state container
    // Kotlin Mutex doesn't hold data like Rust Mutex. We need a wrapper.
    // Simplified:
    val outputState: OutputBufferStateWrapper,
    val outputNotify: Any, // Notify mechanism
    val cancellationToken: Job
)

class OutputBufferStateWrapper {
    val mutex = Mutex()
    val state = OutputBufferState()
}

class UnifiedExecSession(
    private val session: ExecCommandSession,
    private val outputBuffer: OutputBufferStateWrapper,
    private val outputNotify: Any, // Placeholder for Notify
    private val cancellationToken: Job,
    private val outputJob: Job,
    private val sandboxType: SandboxType
) {
    companion object {
        fun new(
            session: ExecCommandSession,
            initialOutputRx: ReceiveChannel<ByteArray>,
            sandboxType: SandboxType,
            scope: CoroutineScope
        ): UnifiedExecSession {
            val outputBuffer = OutputBufferStateWrapper()
            val outputNotify = Any() // Placeholder
            val cancellationToken = Job()
            val bufferWrapper = outputBuffer
            
            val outputJob = scope.launch(Dispatchers.IO) {
                try {
                    for (chunk in initialOutputRx) {
                        bufferWrapper.mutex.withLock {
                            bufferWrapper.state.pushChunk(chunk)
                        }
                        // notify listeners
                    }
                } catch (e: Exception) {
                    // Channel closed or error
                    cancellationToken.cancel()
                }
            }

            return UnifiedExecSession(
                session,
                outputBuffer,
                outputNotify,
                cancellationToken,
                outputJob,
                sandboxType
            )
        }

        suspend fun fromSpawned(
            spawned: SpawnedPty,
            sandboxType: SandboxType,
            scope: CoroutineScope
        ): Result<UnifiedExecSession> {
            val managed = new(spawned.session, spawned.outputRx, sandboxType, scope)
            
            // Check if already exited
            // In Kotlin channels, we can check isClosedForReceive but it might not be immediate.
            // We'll rely on the exitRx.
            
            scope.launch {
                try {
                    spawned.exitRx.receive()
                    managed.signalExit()
                } catch (e: Exception) {
                    managed.signalExit()
                }
            }

            return Result.success(managed)
        }
    }

    fun writerSender(): SendChannel<ByteArray> = session.writerSender()

    suspend fun writeStdin(data: String) {
        try {
            writerSender().send(data.encodeToByteArray())
        } catch (e: Exception) {
            throw UnifiedExecError.WriteToStdin()
        }
    }

    fun outputHandles(): OutputHandles {
        return OutputHandles(
            outputBuffer = outputBuffer.mutex,
            outputBufferState = outputBuffer.state, // This is unsafe without lock, but structure requires it?
            // Rust returns Arc<Mutex<State>>.
            outputState = outputBuffer,
            outputNotify = outputNotify,
            cancellationToken = cancellationToken
        )
    }

    fun hasExited(): Boolean = session.hasExited()
    fun exitCode(): Int? = session.exitCode()

    suspend fun snapshotOutput(): List<ByteArray> {
        return outputBuffer.mutex.withLock {
            outputBuffer.state.snapshot()
        }
    }

    fun sandboxType(): SandboxType = sandboxType

// ...

    suspend fun checkForSandboxDenial(): Result<Unit> {
        if (sandboxType == SandboxType.None || !hasExited()) {
            return Result.success(Unit)
        }

        // Wait briefly for output to flush
        // tokio::time::timeout(Duration::from_millis(20), ...)
        // delay(20)

        val collectedChunks = snapshotOutput()
        val aggregated = collectedChunks.fold(ByteArray(0)) { acc, bytes -> acc + bytes }
        val aggregatedText = aggregated.decodeToString()
        val exitCode = exitCode() ?: -1

        val execOutput = ExecToolCallOutput(
            exitCode = exitCode,
            stdout = StreamOutput(aggregatedText),
            stderr = StreamOutput(""),
            aggregatedOutput = StreamOutput(aggregatedText),
            duration = kotlin.time.Duration.ZERO,
            timedOut = false
        )

        val isDenied = isLikelySandboxDenied(sandboxType, execOutput)

        if (isDenied) {
            val snippet = formattedTruncateText(
                aggregatedText,
                TruncationPolicy.Tokens(UNIFIED_EXEC_OUTPUT_MAX_TOKENS)
            )
            val message = if (snippet.isEmpty()) "exit code $exitCode" else snippet
            return Result.failure(UnifiedExecError.SandboxDenied(message, execOutput))
        }

        return Result.success(Unit)
    }

    fun signalExit() {
        cancellationToken.cancel()
    }
}
