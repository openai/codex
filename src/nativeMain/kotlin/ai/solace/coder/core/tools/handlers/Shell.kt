// port-lint: source core/src/tools/handlers/shell.rs
package ai.solace.coder.core.tools.handlers

import ai.solace.coder.core.exec.ExecParams
import ai.solace.coder.core.exec.ExecExpiration
import ai.solace.coder.core.session.Session
import ai.solace.coder.core.session.TurnContext
import ai.solace.coder.core.tools.ToolHandler
import ai.solace.coder.core.tools.ToolKind
import ai.solace.coder.core.tools.ToolPayload
import ai.solace.coder.core.tools.ToolInvocation
import ai.solace.coder.core.tools.ToolOutput
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.tools.SharedTurnDiffTracker
import ai.solace.coder.protocol.ShellToolCallParams
import ai.solace.coder.protocol.ShellCommandToolCallParams
import kotlinx.serialization.json.Json
import ai.solace.coder.core.command_safety.isKnownSafeCommand

class ShellHandler : ToolHandler {
    override val kind: ToolKind = ToolKind.Function

    override fun matchesKind(payload: ToolPayload): Boolean {
        return payload is ToolPayload.Function || payload is ToolPayload.LocalShell
    }

    override fun isMutating(invocation: ToolInvocation): Boolean {
        return when (val payload = invocation.payload) {
            is ToolPayload.Function -> {
                try {
                    val params = Json.decodeFromString<ShellToolCallParams>(payload.arguments)
                    !isKnownSafeCommand(params.command)
                } catch (e: Exception) {
                    true
                }
            }
            is ToolPayload.LocalShell -> !isKnownSafeCommand(payload.params.command)
            else -> true
        }
    }

    override suspend fun handle(invocation: ToolInvocation): CodexResult<ToolOutput> {
        val payload = invocation.payload
        
        return when (payload) {
            is ToolPayload.Function -> {
                try {
                    val params = Json.decodeFromString<ShellToolCallParams>(payload.arguments)
                    val execParams = toExecParams(params, invocation.turn)
                    runExecLike(
                        invocation.toolName,
                        execParams,
                        invocation.session,
                        invocation.turn,
                        invocation.tracker,
                        invocation.callId,
                        false
                    )
                } catch (e: Exception) {
                    CodexResult.failure(CodexError.RespondToModel("failed to parse function arguments: ${e.message}"))
                }
            }
            is ToolPayload.LocalShell -> {
                val execParams = toExecParams(payload.params, invocation.turn)
                runExecLike(
                    invocation.toolName,
                    execParams,
                    invocation.session,
                    invocation.turn,
                    invocation.tracker,
                    invocation.callId,
                    false
                )
            }
            else -> CodexResult.failure(CodexError.RespondToModel("unsupported payload for shell handler: ${invocation.toolName}"))
        }
    }

    companion object {
        fun toExecParams(params: ShellToolCallParams, turnContext: TurnContext): ExecParams {
            return ExecParams(
                command = params.command,
                cwd = turnContext.resolvePath(params.workdir),
                expiration = ExecExpiration.fromTimeoutMs(params.timeoutMs),
                env = createEnv(turnContext.shellEnvironmentPolicy),
                withEscalatedPermissions = params.withEscalatedPermissions,
                justification = params.justification,
                arg0 = null
            )
        }

        suspend fun runExecLike(
            toolName: String,
            execParams: ExecParams,
            session: Session,
            turn: TurnContext,
            tracker: SharedTurnDiffTracker,
            callId: String,
            freeform: Boolean
        ): CodexResult<ToolOutput> {
            val exec = ai.solace.coder.core.Exec()
            val runtime = ai.solace.coder.core.tools.runtimes.ShellRuntime(exec)
            val orchestrator = ai.solace.coder.core.tools.ToolOrchestrator()
            
            val approvalRequirement = ai.solace.coder.core.tools.defaultApprovalRequirement(
                turn.approvalPolicy,
                turn.sandboxPolicy
            )

            val request = ai.solace.coder.core.tools.runtimes.ShellRequest(
                command = execParams.command,
                cwd = execParams.cwd,
                timeoutMs = execParams.expiration.let { 
                    if (it is ExecExpiration.Timeout) it.duration.inWholeMilliseconds else null 
                },
                env = execParams.env,
                withEscalatedPermissions = execParams.withEscalatedPermissions,
                justification = execParams.justification,
                approvalRequirement = approvalRequirement
            )
            
            val toolCtx = ai.solace.coder.core.tools.ToolCtx(
                session = session,
                turn = turn,
                toolName = toolName,
                callId = callId
            )

            val result = orchestrator.run(
                tool = runtime,
                req = request,
                toolCtx = toolCtx,
                turnCtx = turn,
                approvalPolicy = turn.approvalPolicy
            )
            
            return result.fold(
                onSuccess = { output ->
                    CodexResult.success(ToolOutput.Exec(output))
                },
                onFailure = { error ->
                    val msg = error.message ?: "Unknown error"
                    CodexResult.failure(CodexError.ToolError(msg))
                }
            )
        }
    }
}

class ShellCommandHandler : ToolHandler {
    override val kind: ToolKind = ToolKind.Function

    override fun matchesKind(payload: ToolPayload): Boolean {
        return payload is ToolPayload.Function
    }

    override suspend fun handle(invocation: ToolInvocation): CodexResult<ToolOutput> {
        val payload = invocation.payload as? ToolPayload.Function ?: return CodexResult.failure(
            CodexError.RespondToModel("unsupported payload for shell_command handler: ${invocation.toolName}")
        )

        return try {
            val params = Json.decodeFromString<ShellCommandToolCallParams>(payload.arguments)
            val execParams = toExecParams(params, invocation.session, invocation.turn)
            ShellHandler.runExecLike(
                invocation.toolName,
                execParams,
                invocation.session,
                invocation.turn,
                invocation.tracker,
                invocation.callId,
                true
            )
        } catch (e: Exception) {
            CodexResult.failure(CodexError.RespondToModel("failed to parse function arguments: ${e.message}"))
        }
    }

    companion object {
        fun toExecParams(
            params: ShellCommandToolCallParams,
            session: Session,
            turnContext: TurnContext
        ): ExecParams {
            val shell = session.userShell()
            val useLoginShell = true
            val command = shell.deriveExecArgs(params.command, useLoginShell)

            return ExecParams(
                command = command,
                cwd = turnContext.resolvePath(params.workdir),
                expiration = ExecExpiration.fromTimeoutMs(params.timeoutMs),
                env = createEnv(turnContext.shellEnvironmentPolicy),
                withEscalatedPermissions = params.withEscalatedPermissions,
                justification = params.justification,
                arg0 = null
            )
        }
    }
}

fun createEnv(policy: Any): Map<String, String> {
    // TODO: Implement ShellEnvironmentPolicy logic
    return emptyMap()
}