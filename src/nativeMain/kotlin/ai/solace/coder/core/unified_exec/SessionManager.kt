// port-lint: source core/src/unified_exec/session_manager.rs
package ai.solace.coder.core.unified_exec

import ai.solace.coder.core.session.Session as CodexSession
import ai.solace.coder.core.session.TurnContext
import ai.solace.coder.core.ExecToolCallOutput
import ai.solace.coder.core.StreamOutput
import ai.solace.coder.core.SandboxType
import ai.solace.coder.core.context.TruncationPolicy
import ai.solace.coder.core.context.formattedTruncateText
import ai.solace.coder.core.context.approxTokenCount
import ai.solace.coder.core.tools.events.ToolEmitter
import ai.solace.coder.core.tools.events.ToolEventCtx
import ai.solace.coder.core.tools.events.ToolEventStage
import ai.solace.coder.core.tools.events.ToolEventFailure
import ai.solace.coder.core.tools.events.ExecCommandSource
import ai.solace.coder.core.tools.orchestrator.ToolOrchestrator
import ai.solace.coder.core.tools.runtimes.UnifiedExecRuntime
import ai.solace.coder.core.tools.runtimes.UnifiedExecRequest
import ai.solace.coder.core.sandboxing.SandboxManager
import ai.solace.coder.core.tools.ToolCtx
import ai.solace.coder.core.sandboxing.SandboxPermissions
// import ai.solace.coder.core.tools.createApprovalRequirementForCommand // TODO: Implement this
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.datetime.Clock
import kotlinx.datetime.Instant
import kotlinx.coroutines.SupervisorJob
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds

data class SessionEntry(
    val session: UnifiedExecSession,
    val sessionRef: CodexSession,
    val turnRef: TurnContext,
    val callId: String,
    val processId: String,
    val command: List<String>,
    val cwd: String,
    val startedAt: Instant,
    var lastUsed: Instant
)

sealed class SessionStatus {
    data class Alive(
        val exitCode: Int?,
        val callId: String,
        val processId: String
    ) : SessionStatus()

    data class Exited(
        val exitCode: Int?,
        val entry: SessionEntry
    ) : SessionStatus()

    object Unknown : SessionStatus()
}

class UnifiedExecSessionManager {
    private val sessions = Mutex() // Guards HashMap<String, SessionEntry>
    private val sessionsMap = HashMap<String, SessionEntry>()
    
    private val usedSessionIds = Mutex() // Guards HashSet<String>
    private val usedSessionIdsSet = HashSet<String>()

    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())

    suspend fun allocateProcessId(): String {
        while (true) {
            usedSessionIds.withLock {
                // Simple random ID generation for now
                val processId = (1000..100000).random().toString()
                if (!usedSessionIdsSet.contains(processId)) {
                    usedSessionIdsSet.add(processId)
                    return processId
                }
            }
        }
    }

    suspend fun execCommand(
        request: ExecCommandRequest,
        context: UnifiedExecContext
    ): UnifiedExecResponse {
        val cwd = request.workdir ?: context.turn.cwd

        val session = openSessionWithSandbox(
            request.command,
            cwd,
            request.withEscalatedPermissions,
            request.justification,
            context
        )

        val maxTokens = resolveMaxTokens(request.maxOutputTokens)
        val yieldTimeMs = clampYieldTime(request.yieldTimeMs)

        val start = Clock.System.now()
        val handles = session.outputHandles()
        val deadline = start + yieldTimeMs.milliseconds
        
        val collected = collectOutputUntilDeadline(
            handles.outputState, // Pass wrapper
            handles.outputNotify,
            handles.cancellationToken,
            deadline
        )
        
        val wallTime = Clock.System.now() - start

        val text = collected.toByteArray().decodeToString()
        val output = formattedTruncateText(text, TruncationPolicy.Tokens(maxTokens))
        val hasExited = session.hasExited()
        val exitCode = session.exitCode()
        val chunkId = generateChunkId()
        
        val processId = if (hasExited) {
            null
        } else {
            storeSession(
                session,
                context,
                request.command,
                cwd,
                start,
                request.processId
            )
            request.processId
        }
        
        val originalTokenCount = approxTokenCount(text)

        val response = UnifiedExecResponse(
            eventCallId = context.callId,
            chunkId = chunkId,
            wallTime = wallTime,
            output = output,
            processId = processId,
            exitCode = exitCode,
            originalTokenCount = originalTokenCount,
            sessionCommand = request.command
        )

        if (!hasExited) {
            emitWaitingStatus(context.session, context.turn, request.command)
        }

        if (hasExited) {
            val exit = response.exitCode ?: -1
            emitExecEndFromContext(
                context,
                request.command,
                cwd,
                response.output,
                exit,
                response.wallTime,
                request.processId
            )
        }

        return response
    }

    suspend fun writeStdin(processId: String, data: String) {
        val entry = sessions.withLock {
            sessionsMap[processId]
        } ?: throw UnifiedExecError.UnknownSessionId(processId)
        
        entry.lastUsed = Clock.System.now()
        entry.session.writeStdin(data)
    }

    suspend fun openSessionWithExecEnv(
        env: ai.solace.coder.core.ExecEnv
    ): UnifiedExecSession {
        val (program, args) = if (env.command.isNotEmpty()) {
            env.command.first() to env.command.drop(1)
        } else {
            throw UnifiedExecError.MissingCommandLine
        }

        // Use PTY lib to spawn process
        val spawned = ai.solace.coder.utils.pty.spawnPtyProcess(
            program,
            args,
            env.cwd,
            env.env,
            scope
        )

        return UnifiedExecSession.fromSpawned(spawned, env.sandbox, scope).getOrThrow()
    }

    suspend fun openSessionWithSandbox(
        command: List<String>,
        cwd: String,
        withEscalatedPermissions: Boolean?,
        justification: String?,
        context: UnifiedExecContext
    ): UnifiedExecSession {
        if (command.isEmpty()) {
            throw UnifiedExecError.MissingCommandLine
        }
        val (program, args) = command.first() to command.drop(1)

        val spec = ai.solace.coder.core.CommandSpec(
            program = program,
            args = args,
            cwd = cwd,
            env = emptyMap(), // TODO: Inherit or config
            expiration = ai.solace.coder.core.ExecExpiration.DefaultTimeout, // Unified execs are usually interactive/long-running?
            withEscalatedPermissions = withEscalatedPermissions,
            justification = justification
        )

        // Default policy for unified exec (terminal) is usually permissive or standard
        val policy = ai.solace.coder.protocol.SandboxPolicy.DangerFullAccess 
        val sandboxType = ai.solace.coder.core.platformGetSandbox() ?: ai.solace.coder.core.SandboxType.None
        
        val manager = SandboxManager()
        val transformResult = manager.transform(
            spec,
            policy,
            cwd
        )

        if (transformResult.isFailure()) {
            throw UnifiedExecError.SandboxError(transformResult.exceptionOrNull()?.message ?: "Sandbox transform failed")
        }

        val execEnv = transformResult.getOrThrow()
        return openSessionWithExecEnv(execEnv)
    }

    private suspend fun collectOutputUntilDeadline(
        outputBuffer: OutputBufferStateWrapper,
        outputNotify: Any,
        cancellationToken: Job,
        deadline: Instant
    ): List<Byte> {
        // Simplified implementation
        val collected = ArrayList<Byte>()
        
        // Loop until deadline or exit
        while (Clock.System.now() < deadline) {
             val chunks = outputBuffer.mutex.withLock {
                 outputBuffer.state.drain()
             }
             
             if (chunks.isNotEmpty()) {
                 chunks.forEach { collected.addAll(it.toList()) }
             } else {
                 if (cancellationToken.isCancelled) break
                 delay(10) // Poll
             }
        }
        return collected
    }

    private suspend fun storeSession(
        session: UnifiedExecSession,
        context: UnifiedExecContext,
        command: List<String>,
        cwd: String,
        startedAt: Instant,
        processId: String
    ) {
        val entry = SessionEntry(
            session,
            context.session,
            context.turn,
            context.callId,
            processId,
            command,
            cwd,
            startedAt,
            startedAt
        )
        sessions.withLock {
            sessionsMap[processId] = entry
        }
    }

    private suspend fun emitWaitingStatus(
        session: CodexSession,
        turn: TurnContext,
        command: List<String>
    ) {
        // Emit event
    }

    private suspend fun emitExecEndFromContext(
        context: UnifiedExecContext,
        command: List<String>,
        cwd: String,
        aggregatedOutput: String,
        exitCode: Int,
        duration: Duration,
        processId: String?
    ) {
        // Emit event
    }
}
