// port-lint: source core/src/tools/runtimes/shell.rs
package ai.solace.coder.core.tools.runtimes

import ai.solace.coder.core.tools.Approvable
import ai.solace.coder.core.session.SessionServices
import ai.solace.coder.core.tools.ApprovalCtx
import ai.solace.coder.core.tools.ApprovalRequirement
import ai.solace.coder.core.tools.SandboxAttempt
import ai.solace.coder.core.tools.Sandboxable
import ai.solace.coder.core.tools.SandboxablePreference
import ai.solace.coder.core.tools.ToolCtx
import ai.solace.coder.core.tools.ToolError
import ai.solace.coder.core.tools.ToolRuntime
import ai.solace.coder.core.tools.ProvidesSandboxRetryData
import ai.solace.coder.core.tools.SandboxRetryData
import ai.solace.coder.core.tools.SandboxOverride
import ai.solace.coder.core.tools.withCachedApproval
import ai.solace.coder.protocol.ReviewDecision
import ai.solace.coder.core.ExecToolCallOutput
import ai.solace.coder.core.ExecExpiration
import ai.solace.coder.core.StdoutStream
import ai.solace.coder.core.Exec
import ai.solace.coder.core.ExecEnv
import ai.solace.coder.core.tools.buildCommandSpec
import ai.solace.coder.core.unified_exec.Errors.SandboxError
import ai.solace.coder.core.error.CodexError
import kotlin.time.Duration.Companion.milliseconds

data class ShellRequest(
    val command: List<String>,
    val cwd: String,
    val timeoutMs: Long?,
    val env: Map<String, String>,
    val withEscalatedPermissions: Boolean?,
    val justification: String?,
    val approvalRequirement: ApprovalRequirement
) : ProvidesSandboxRetryData {
    override fun sandboxRetryData(): SandboxRetryData? {
        return SandboxRetryData(
            command = command,
            cwd = cwd
        )
    }
}

data class ShellApprovalKey(
    val command: List<String>,
    val cwd: String,
    val escalated: Boolean
)

class ShellRuntime(
    private val processExecutor: Exec
) : ToolRuntime<ShellRequest, ExecToolCallOutput>, Sandboxable, Approvable<ShellRequest> {

    override fun sandboxPreference(): SandboxablePreference {
        return SandboxablePreference.Auto
    }

    override fun escalateOnFailure(): Boolean {
        return true
    }

    override fun approvalKey(req: ShellRequest): Any {
        return ShellApprovalKey(
            command = req.command,
            cwd = req.cwd,
            escalated = req.withEscalatedPermissions ?: false
        )
    }

    override suspend fun startApprovalAsync(req: ShellRequest, ctx: ApprovalCtx): ReviewDecision {
        val key = approvalKey(req)
        val session = ctx.session
        val turn = ctx.turn
        val callId = ctx.callId
        val command = req.command
        val cwd = req.cwd
        val reason = ctx.retryReason ?: req.justification
        val risk = ctx.risk

        return withCachedApproval(session.services, key) {
            session.requestCommandApproval(
                turn,
                callId,
                command,
                cwd,
                reason,
                risk
            )
        }
    }

    override fun approvalRequirement(req: ShellRequest): ApprovalRequirement? {
        return req.approvalRequirement
    }

    override fun sandboxModeForFirstAttempt(req: ShellRequest): SandboxOverride {
        val bypass = req.withEscalatedPermissions == true || 
                     (req.approvalRequirement is ApprovalRequirement.Skip && req.approvalRequirement.bypassSandbox)
        
        return if (bypass) {
            SandboxOverride.BypassSandboxFirstAttempt
        } else {
            SandboxOverride.NoOverride
        }
    }

    override suspend fun run(
        req: ShellRequest,
        attempt: SandboxAttempt,
        ctx: ToolCtx
    ): Result<ExecToolCallOutput> {
        val spec = buildCommandSpec(
            req.command,
            req.cwd,
            req.env,
            if (req.timeoutMs != null) ExecExpiration.Timeout(req.timeoutMs.milliseconds) else ExecExpiration.DefaultTimeout,
            req.withEscalatedPermissions,
            req.justification
        ).getOrElse { return Result.failure(ToolError.Rejected(it.message ?: "Invalid command spec")) }

        val env = attempt.envFor(spec)
            .getOrElse { return Result.failure(ToolError.Codex(it.message ?: "Unknown error")) }

        return try {
            executeEnv(env, attempt.policy, stdoutStream(ctx))
        } catch (e: Exception) {
            Result.failure(ToolError.Codex(CodexError.Sandbox(SandboxError(e.message ?: "Execution failed"))))
        }
    }

    private fun stdoutStream(ctx: ToolCtx): StdoutStream {
        return StdoutStream(
            subId = ctx.turn.subId,
            callId = ctx.callId,
            txEvent = ctx.session.getTxEvent()
        )
    }
    
    // Helper to execute env using ProcessExecutor
    private suspend fun executeEnv(
        env: ExecEnv, 
        policy: ai.solace.coder.protocol.SandboxPolicy,
        stdoutStream: StdoutStream?
    ): Result<ExecToolCallOutput> {
         val result = processExecutor.executeExecEnv(env, policy, stdoutStream)
         return when (result) {
             is ai.solace.coder.core.error.CodexResult.Success -> Result.success(result.value)
             is ai.solace.coder.core.error.CodexResult.Failure -> Result.failure(result.error.toException())
         }
    }
}
