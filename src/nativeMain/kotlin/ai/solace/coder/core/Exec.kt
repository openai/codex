// port-lint: source core/src/exec.rs
package ai.solace.coder.core

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.core.sandboxing.SandboxManager
import ai.solace.coder.exec.shell.ShellDetector
import ai.solace.coder.protocol.SandboxPolicy
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.SendChannel
import kotlinx.coroutines.withContext
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.measureTime

/**
 * Configuration for process execution
 */
data class ExecParams(
    val command: List<String>,
    val cwd: String,
    val expiration: ExecExpiration,
    val env: Map<String, String> = emptyMap(),
    val withEscalatedPermissions: Boolean? = null,
    val justification: String? = null,
    val arg0: String? = null
)

/**
 * Mechanism to terminate an exec invocation before it finishes naturally
 */
sealed class ExecExpiration {
    data class Timeout(val duration: kotlin.time.Duration) : ExecExpiration()
    object DefaultTimeout : ExecExpiration()
    data class Cancellation(val cancelToken: Job) : ExecExpiration()

    companion object {
        fun fromTimeoutMs(timeoutMs: Long?): ExecExpiration {
            return if (timeoutMs != null) {
                Timeout(timeoutMs.milliseconds)
            } else {
                DefaultTimeout
            }
        }
    }
}

/**
 * Output streams from process execution
 */
data class StreamOutput<T>(
    val text: T,
    val truncatedAfterLines: UInt? = null
)

/**
 * Simple process result for internal command execution (e.g., git commands)
 */
data class SimpleProcessResult(
    val exitCode: Int,
    val stdout: String,
    val stderr: String
)

/**
 * Result of process execution
 */
data class ExecToolCallOutput(
    val exitCode: Int,
    val stdout: StreamOutput<String>,
    val stderr: StreamOutput<String>,
    val aggregatedOutput: StreamOutput<String>,
    val duration: kotlin.time.Duration,
    val timedOut: Boolean
)

/**
 * Raw process output before UTF-8 conversion
 */
private data class RawExecToolCallOutput(
    val exitStatus: Int,
    val stdout: StreamOutput<ByteArray>,
    val stderr: StreamOutput<ByteArray>,
    val aggregatedOutput: StreamOutput<ByteArray>,
    val timedOut: Boolean
)

/**
 * Stream for stdout events
 */
data class StdoutStream(
    val subId: String,
    val callId: String,
    val txEvent: SendChannel<ai.solace.coder.protocol.Event>
)

/**
 * Main process executor with timeout and streaming support
 */
/**
 * Main process executor with timeout and streaming support
 */
class Exec {
    companion object {
        private const val DEFAULT_EXEC_COMMAND_TIMEOUT_MS = 10_000L
        private const val SIGKILL_CODE = 9
        private const val TIMEOUT_CODE = 64
        private const val EXIT_CODE_SIGNAL_BASE = 128
        private const val EXEC_TIMEOUT_EXIT_CODE = 124
        private const val READ_CHUNK_SIZE = 8192
        private const val AGGREGATE_BUFFER_INITIAL_CAPACITY = 8 * 1024
        private const val MAX_EXEC_OUTPUT_DELTAS_PER_CALL = 10_000
        private const val IO_DRAIN_TIMEOUT_MS = 2_000L
    }

    private val sandboxManager = SandboxManager()
    private val shellDetector = ShellDetector()

    /**
     * Execute a command with sandboxing and streaming output
     */
    suspend fun execute(
        params: ExecParams,
        sandboxPolicy: SandboxPolicy,
        sandboxCwd: String,
        stdoutStream: StdoutStream? = null
    ): CodexResult<ExecToolCallOutput> = withContext(Dispatchers.Default) {
        try {
            val (program, args) = params.command.splitFirst()
                ?: return@withContext CodexResult.failure(
                    CodexError.Io("command args are empty")
                )

            val spec = CommandSpec(
                program = program,
                args = args,
                cwd = params.cwd,
                env = params.env,
                expiration = params.expiration,
                withEscalatedPermissions = params.withEscalatedPermissions,
                justification = params.justification
            )

            val transformResult = sandboxManager.transform(
                spec,
                sandboxPolicy,
                sandboxCwd
            )
            if (transformResult.isFailure()) {
                return@withContext CodexResult.failure(CodexError.Io("Process setup failed"))
            }
            val execEnv = transformResult.getOrThrow()

            lateinit var rawOutput: RawExecToolCallOutput
            val duration = measureTime {
                rawOutput = executeEnv(execEnv, sandboxPolicy, stdoutStream)
            }
            val output = finalizeExecResult(rawOutput, execEnv.sandbox, duration)
            
            CodexResult.success(output)
        } catch (e: Exception) {
            CodexResult.failure(CodexError.Io("Process execution failed: ${e.message}"))
        }
    }

    /**
     * Simple command execution for internal tools like git.
     * Returns a basic result with exit code, stdout, and stderr.
     */
    suspend fun executeCommand(
        executable: String,
        args: List<String>,
        cwd: String,
        timeout: kotlin.time.Duration = kotlin.time.Duration.parse("10s")
    ): SimpleProcessResult {
        val command = listOf(executable) + args
        val params = ExecParams(
            command = command,
            cwd = cwd,
            expiration = ExecExpiration.Timeout(timeout)
        )

        val result = execute(
            params = params,
            sandboxPolicy = SandboxPolicy.ReadOnly,
            sandboxCwd = cwd
        )

        return result.fold(
            onSuccess = { output ->
                SimpleProcessResult(
                    exitCode = output.exitCode,
                    stdout = output.stdout.text,
                    stderr = output.stderr.text
                )
            },
            onFailure = { error ->
                // Non-zero exit code with error in stderr so caller sees what went wrong
                SimpleProcessResult(1, "", error.toString())
            }
        )
    }

    /**
     * Execute the transformed environment
     */
    /**
     * Execute the transformed environment and return finalized output
     */
    suspend fun executeExecEnv(
        env: ExecEnv,
        sandboxPolicy: SandboxPolicy,
        stdoutStream: StdoutStream?
    ): CodexResult<ExecToolCallOutput> = withContext(Dispatchers.Default) {
        try {
            lateinit var rawOutput: RawExecToolCallOutput
            val duration = measureTime {
                rawOutput = executeEnv(env, sandboxPolicy, stdoutStream)
            }
            val output = finalizeExecResult(rawOutput, env.sandbox, duration)
            CodexResult.success(output)
        } catch (e: Exception) {
            CodexResult.failure(CodexError.Io("Process execution failed: ${e.message}"))
        }
    }

    /**
     * Execute the transformed environment (internal, raw output)
     */
    private suspend fun executeEnv(
        env: ExecEnv,
        sandboxPolicy: SandboxPolicy,
        stdoutStream: StdoutStream?
    ): RawExecToolCallOutput {
        return exec(env.command, env.cwd, env.env, env.expiration, stdoutStream)
    }

    /**
     * Core execution logic
     */
    private suspend fun exec(
        command: List<String>,
        cwd: String,
        env: Map<String, String>,
        expiration: ExecExpiration,
        stdoutStream: StdoutStream?
    ): RawExecToolCallOutput {
        val splitCommand = command.splitFirst()
            ?: throw IllegalArgumentException("command args are empty")
        val program = splitCommand.first
        val args = splitCommand.second

        // Create process using platform-specific APIs
        val process = createProcess(program, args, cwd, env)
        
        return consumeTruncatedOutput(process, expiration, stdoutStream)
    }

    /**
     * Create a platform-specific process
     */
    private fun createProcess(
        program: String,
        args: List<String>,
        cwd: String,
        env: Map<String, String>
    ): ProcessHandle {
        return platformCreateProcess(program, args, cwd, env)
    }

    /**
     * Consume process output with truncation and timeout
     */
    private suspend fun consumeTruncatedOutput(
        process: ProcessHandle,
        expiration: ExecExpiration,
        stdoutStream: StdoutStream?
    ): RawExecToolCallOutput {
        // Wait for process completion or timeout
        val (exitStatus, timedOut) = when (expiration) {
            is ExecExpiration.Timeout -> {
                // Simple timeout implementation - wait for process with timeout
                try {
                    val result = kotlinx.coroutines.withTimeout(expiration.duration.inWholeMilliseconds) {
                        process.onAwait()
                    }
                    Pair(result, false)
                } catch (_: kotlinx.coroutines.TimeoutCancellationException) {
                    killChildProcessGroup(process)
                    Pair(EXIT_CODE_SIGNAL_BASE + TIMEOUT_CODE, true)
                }
            }
            is ExecExpiration.DefaultTimeout -> {
                // Default timeout implementation
                try {
                    val result = kotlinx.coroutines.withTimeout(DEFAULT_EXEC_COMMAND_TIMEOUT_MS) {
                        process.onAwait()
                    }
                    Pair(result, false)
                } catch (_: kotlinx.coroutines.TimeoutCancellationException) {
                    killChildProcessGroup(process)
                    Pair(EXIT_CODE_SIGNAL_BASE + TIMEOUT_CODE, true)
                }
            }
            is ExecExpiration.Cancellation -> {
                // Cancellation implementation
                try {
                    process.onAwait()
                    Pair(0, false) // Success if not cancelled
                } catch (_: kotlinx.coroutines.CancellationException) {
                    killChildProcessGroup(process)
                    Pair(EXIT_CODE_SIGNAL_BASE + SIGKILL_CODE, false)
                }
            }
        }

        // Get output from process (simplified - no streaming for now)
        val stdout = StreamOutput(
            text = process.stdout ?: byteArrayOf(),
            truncatedAfterLines = null
        )
        val stderr = StreamOutput(
            text = process.stderr ?: byteArrayOf(),
            truncatedAfterLines = null
        )

        // Aggregate output
        val aggregatedOutput = StreamOutput(
            text = (process.stdout ?: byteArrayOf()) + (process.stderr ?: byteArrayOf()),
            truncatedAfterLines = null
        )

        return RawExecToolCallOutput(
            exitStatus = exitStatus,
            stdout = stdout,
            stderr = stderr,
            aggregatedOutput = aggregatedOutput,
            timedOut = timedOut
        )
    }



    /**
     * Kill child process group (Unix-specific)
     */
    private fun killChildProcessGroup(process: ProcessHandle) {
        platformKillChildProcessGroup(process)
    }

    /**
     * Finalize execution result and handle sandbox detection
     */
    private suspend fun finalizeExecResult(
        rawOutput: RawExecToolCallOutput,
        sandboxType: SandboxType,
        duration: kotlin.time.Duration
    ): ExecToolCallOutput {
        val timedOut = rawOutput.timedOut
        var exitCode = rawOutput.exitStatus

        // Handle timeout exit code
        if (timedOut) {
            exitCode = EXEC_TIMEOUT_EXIT_CODE
        }

        // Convert UTF-8 output
        val stdout = rawOutput.stdout.fromUtf8Lossy()
        val stderr = rawOutput.stderr.fromUtf8Lossy()
        val aggregatedOutput = rawOutput.aggregatedOutput.fromUtf8Lossy()

        val execOutput = ExecToolCallOutput(
            exitCode = exitCode,
            stdout = stdout,
            stderr = stderr,
            aggregatedOutput = aggregatedOutput,
            duration = duration,
            timedOut = timedOut
        )

        // Check for sandbox denial
        if (isLikelySandboxDenied(sandboxType, execOutput)) {
            // Convert to exception so caller's try/catch will turn it into a failure CodexResult
            throw ai.solace.coder.core.error.CodexException(
                CodexError.SandboxError.ApplicationFailed("Sandbox denied execution")
            )
        }

        return execOutput
    }

    /**
     * Platform-specific process creation
     */
    private fun platformCreateProcess(
        program: String,
        args: List<String>,
        cwd: String,
        env: Map<String, String>
    ): ProcessHandle {
        // This will be implemented with expect/actual
        return createPlatformProcess(program, args, cwd, env)
    }

    /**
     * Platform-specific process group killing
     */
    private fun platformKillChildProcessGroup(process: ProcessHandle) {
        // This will be implemented with expect/actual
        killPlatformChildProcessGroup(process)
    }
}

/**
 * Check if execution likely failed due to sandbox restrictions
 */
fun isLikelySandboxDenied(
    sandboxType: SandboxType, // Added sandboxType arg to match usage in Sandboxing.kt if needed, or just check output
    execOutput: ExecToolCallOutput
): Boolean {
    // Note: The original private method didn't take sandboxType, but Rust does.
    // Sandboxing.kt calls it with (sandbox, output).
    // So I should update the signature to match Rust and Sandboxing.kt usage.
    
    if (sandboxType == SandboxType.None || execOutput.exitCode == 0) return false

    // Quick rejects: well-known non-sandbox shell exit codes
    val quickRejectExitCodes = setOf(2, 126, 127)
    if (quickRejectExitCodes.contains(execOutput.exitCode)) return false

    val sandboxDeniedKeywords = listOf(
        "operation not permitted",
        "permission denied", 
        "read-only file system",
        "seccomp",
        "sandbox",
        "landlock",
        "failed to write file"
    )

    val hasSandboxKeyword = listOf(
        execOutput.stderr.text,
        execOutput.stdout.text,
        execOutput.aggregatedOutput.text
    ).any { section ->
        section.lowercase().let { lower ->
            sandboxDeniedKeywords.any { keyword -> lower.contains(keyword) }
        }
    }

    return hasSandboxKeyword
}

/**
 * Overload for internal usage if needed, or just update internal usage.
 * The internal usage in finalizeExecResult didn't pass sandboxType.
 * I need to update finalizeExecResult to pass sandboxType if I change the signature.
 * But finalizeExecResult doesn't have access to sandboxType in the current structure easily unless passed down.
 * In Rust, finalize_exec_result takes sandbox_type.
 * In Kotlin Exec.kt, finalizeExecResult is called from execute.
 * execute has sandboxEnv which contains sandbox type? No, executeEnv returns RawExecToolCallOutput.
 * execute has `execEnv` which has `sandbox` field (SandboxType).
 * So I can pass `execEnv.sandbox` to finalizeExecResult.
 */

/**
 * Extension function to split list into first element and rest
 */
private fun <T> List<T>.splitFirst(): Pair<T, List<T>>? {
    return if (isEmpty()) null else first() to drop(1)
}

/**
 * Extension function to convert ByteArray to UTF-8 string with lossy conversion
 */
private fun StreamOutput<ByteArray>.fromUtf8Lossy(): StreamOutput<String> {
    return StreamOutput(
        text = text.decodeToString(),
        truncatedAfterLines = truncatedAfterLines
    )
}

/**
 * Extension function for ByteArray concatenation
 */
private operator fun ByteArray.plus(other: ByteArray): ByteArray {
    val result = ByteArray(this.size + other.size)
    this.copyInto(result, 0, 0, this.size)
    other.copyInto(result, this.size, 0, other.size)
    return result
}

/**
 * Extension function for List<ByteArray> to ByteArray
 */
private fun List<ByteArray>.toByteArray(): ByteArray {
    val totalSize = sumOf { it.size }
    val result = ByteArray(totalSize)
    var offset = 0
    for (chunk in this) {
        chunk.copyInto(result, offset)
        offset += chunk.size
    }
    return result
}

/**
 * Process handle abstraction for platform-specific implementations
 */

/**
 * Command specification for sandbox transformation
 */
data class CommandSpec(
    val program: String,
    val args: List<String>,
    val cwd: String,
    val env: Map<String, String>,
    val expiration: ExecExpiration,
    val withEscalatedPermissions: Boolean?,
    val justification: String?
)

/**
 * Execution environment after sandbox transformation
 */
data class ExecEnv(
    val command: List<String>,
    val cwd: String,
    val env: Map<String, String>,
    val expiration: ExecExpiration,
    val sandbox: SandboxType,
    val withEscalatedPermissions: Boolean?,
    val justification: String?,
    val arg0: String?
)

/**
 * Sandbox type enumeration
 */
enum class SandboxType {
    None,
    MacosSeatbelt,
    LinuxSeccomp,
    WindowsRestrictedToken
}