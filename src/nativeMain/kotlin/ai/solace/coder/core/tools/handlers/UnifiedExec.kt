// port-lint: source core/src/tools/handlers/unified_exec.rs
package ai.solace.coder.core.tools.handlers

import ai.solace.coder.core.tools.ToolHandler
import ai.solace.coder.core.tools.ToolKind
import ai.solace.coder.core.tools.ToolPayload
import ai.solace.coder.core.tools.ToolInvocation
import ai.solace.coder.core.tools.ToolOutput
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.unified_exec.UnifiedExecContext
import ai.solace.coder.core.unified_exec.ExecCommandRequest
import ai.solace.coder.core.unified_exec.WriteStdinRequest
import ai.solace.coder.core.sandboxing.assessCommand
import ai.solace.coder.protocol.SandboxCommandAssessment
import ai.solace.coder.protocol.SandboxPolicy
import kotlinx.serialization.json.Json
import kotlinx.serialization.Serializable

class UnifiedExecHandler : ToolHandler {
    override val kind: ToolKind = ToolKind.Function

    override fun matchesKind(payload: ToolPayload): Boolean {
        return payload is ToolPayload.Function || payload is ToolPayload.UnifiedExec
    }

    override fun isMutating(invocation: ToolInvocation): Boolean {
        val arguments = when (val payload = invocation.payload) {
            is ToolPayload.Function -> payload.arguments
            is ToolPayload.UnifiedExec -> payload.arguments
            else -> return true
        }

        return try {
            val args = Json { ignoreUnknownKeys = true }.decodeFromString<ExecCommandArgs>(arguments)
            val command = getCommand(args)
            val assessment = assessCommand(command, invocation.turn.sandboxPolicy)
            assessment != SandboxCommandAssessment.Low
        } catch (e: Exception) {
            true
        }
    }

    override suspend fun handle(invocation: ToolInvocation): CodexResult<ToolOutput> {
        val arguments = when (val payload = invocation.payload) {
            is ToolPayload.Function -> payload.arguments
            is ToolPayload.UnifiedExec -> payload.arguments
            else -> return CodexResult.failure(CodexError.Fatal("unified_exec handler received unsupported payload"))
        }

        // We need access to the session manager.
        // In Rust: let manager: &UnifiedExecSessionManager = &session.services.unified_exec_manager;
        // In Kotlin, we assume invocation.session has services or we inject it.
        // For now, let's assume we can get it from the session (casting or property).
        // invocation.session is CodexSession.
        
        // Access UnifiedExecSessionManager from CodexSession
        val manager = invocation.session.services.unifiedExecManager

        val context = UnifiedExecContext(invocation.session, invocation.turn, invocation.callId)

        val response = when (invocation.toolName) {
            "exec_command" -> {
                val args = try {
                    Json { ignoreUnknownKeys = true }.decodeFromString<ExecCommandArgs>(arguments)
                } catch (e: Exception) {
                    return CodexResult.failure(CodexError.Fatal("failed to parse exec_command arguments: ${e.message}"))
                }
                
                val processId = manager.allocateProcessId()
                
                // Check permissions
                if (invocation.turn.sandboxPolicy == SandboxPolicy.ReadOnly) {
                    val command = getCommand(args)
                    val assessment = assessCommand(command, invocation.turn.sandboxPolicy)
                    if (assessment != SandboxCommandAssessment.Low) {
                         return CodexResult.failure(CodexError.Sandbox("Command denied by read-only policy"))
                    }
                }
                
                manager.execCommand(
                    ExecCommandRequest(
                        command = getCommand(args),
                        processId = processId,
                        yieldTimeMs = args.yieldTimeMs,
                        maxOutputTokens = args.maxOutputTokens,
                        workdir = args.workdir,
                        withEscalatedPermissions = args.withEscalatedPermissions,
                        justification = args.justification
                    ),
                    context
                )
            }
            "write_stdin" -> {
                val args = try {
                    Json { ignoreUnknownKeys = true }.decodeFromString<WriteStdinRequest>(arguments)
                } catch (e: Exception) {
                    return CodexResult.failure(CodexError.Fatal("failed to parse write_stdin arguments: ${e.message}"))
                }
                
                manager.writeStdin(args.processId, args.data)
                
                UnifiedExecResponse(
                    eventCallId = invocation.callId,
                    chunkId = "",
                    wallTime = kotlin.time.Duration.ZERO,
                    output = "",
                    processId = args.processId,
                    exitCode = null,
                    originalTokenCount = null,
                    sessionCommand = emptyList()
                )
            }
            else -> return CodexResult.failure(CodexError.Fatal("unsupported unified exec function ${invocation.toolName}"))
        }

        // Emit delta event if needed (skipped for now)

        val content = formatResponse(response)

        return CodexResult.success(ToolOutput.Function(
            content = content,
            success = true
        ))
    }

    override fun getTimeoutMs(): Long = 300000L // Default
}

@Serializable
data class ExecCommandArgs(
    val cmd: String,
    val workdir: String? = null,
    val shell: String = "/bin/bash",
    val login: Boolean = true,
    val yieldTimeMs: Long = 10000,
    val maxOutputTokens: Int? = null,
    val withEscalatedPermissions: Boolean? = null,
    val justification: String? = null
)

fun getCommand(args: ExecCommandArgs): List<String> {
    // Simplified shell derivation
    return if (args.login) {
        listOf(args.shell, "-l", "-c", args.cmd)
    } else {
        listOf(args.shell, "-c", args.cmd)
    }
}

fun formatResponse(response: UnifiedExecResponse): String {
    val sections = ArrayList<String>()
    if (response.chunkId.isNotEmpty()) {
        sections.add("Chunk ID: ${response.chunkId}")
    }
    sections.add("Wall time: ${response.wallTime}")
    if (response.exitCode != null) {
        sections.add("Process exited with code ${response.exitCode}")
    }
    if (response.processId != null) {
        sections.add("Process running with session ID ${response.processId}")
    }
    if (response.originalTokenCount != null) {
        sections.add("Original token count: ${response.originalTokenCount}")
    }
    sections.add("Output:")
    sections.add(response.output)
    
    return sections.joinToString("\n")
}