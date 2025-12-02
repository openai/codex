// port-lint: source core/src/tools/runtimes/apply_patch.rs
package ai.solace.coder.core.tools.runtimes

import ai.solace.coder.core.tools.Approvable
import ai.solace.coder.core.tools.ApprovalCtx
import ai.solace.coder.core.tools.SandboxAttempt
import ai.solace.coder.core.tools.Sandboxable
import ai.solace.coder.core.tools.SandboxablePreference
import ai.solace.coder.core.tools.ToolCtx
import ai.solace.coder.core.tools.ToolError
import ai.solace.coder.core.tools.ToolRuntime
import ai.solace.coder.core.tools.ProvidesSandboxRetryData
import ai.solace.coder.core.tools.SandboxRetryData
import ai.solace.coder.core.tools.withCachedApproval
import ai.solace.coder.protocol.AskForApproval
import ai.solace.coder.protocol.ReviewDecision
import ai.solace.coder.core.ExecToolCallOutput
import ai.solace.coder.core.CommandSpec
import ai.solace.coder.core.ExecExpiration
import ai.solace.coder.core.StdoutStream
import ai.solace.coder.core.Exec
import ai.solace.coder.core.ExecEnv
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds

// Constants
const val CODEX_APPLY_PATCH_ARG1 = "--codex-run-as-apply-patch"

data class ApplyPatchRequest(
    val patch: String,
    val cwd: String,
    val timeoutMs: Long?,
    val userExplicitlyApproved: Boolean,
    val codexExe: String?
) : ProvidesSandboxRetryData {
    override fun sandboxRetryData(): SandboxRetryData? = null
}

data class ApprovalKey(
    val patch: String,
    val cwd: String
)

class ApplyPatchRuntime(
    private val processExecutor: Exec
) : ToolRuntime<ApplyPatchRequest, ExecToolCallOutput>, Sandboxable, Approvable<ApplyPatchRequest> {

    override fun sandboxPreference(): SandboxablePreference {
        return SandboxablePreference.Auto
    }

    override fun escalateOnFailure(): Boolean {
        return true
    }

    override fun approvalKey(req: ApplyPatchRequest): Any {
        return ApprovalKey(req.patch, req.cwd)
    }

    override suspend fun startApprovalAsync(req: ApplyPatchRequest, ctx: ApprovalCtx): ReviewDecision {
        val key = approvalKey(req)
        val session = ctx.session
        val turn = ctx.turn
        val callId = ctx.callId
        val cwd = req.cwd
        val retryReason = ctx.retryReason
        val risk = ctx.risk
        val userExplicitlyApproved = req.userExplicitlyApproved

        return withCachedApproval(session.services, key) {
            if (retryReason != null) {
                session.requestCommandApproval(
                    turn,
                    callId,
                    listOf("apply_patch"),
                    cwd,
                    retryReason,
                    risk
                )
            } else if (userExplicitlyApproved) {
                ReviewDecision.ApprovedForSession
            } else {
                ReviewDecision.Approved
            }
        }
    }

    override fun wantsNoSandboxApproval(policy: AskForApproval): Boolean {
        return policy != AskForApproval.Never
    }

    override suspend fun run(
        req: ApplyPatchRequest,
        attempt: SandboxAttempt,
        ctx: ToolCtx
    ): Result<ExecToolCallOutput> {
        val spec = buildCommandSpec(req).getOrElse { return Result.failure(it) }
        
        val env = attempt.envFor(spec)
            .getOrElse { return Result.failure(ToolError.Codex(it.message ?: "Unknown error")) }
            
        return executeEnv(env, attempt.policy, stdoutStream(ctx))
            .mapCatching { it }
            .recoverCatching { throw ToolError.Codex(it.message ?: "Execution failed") }
    }

    private fun buildCommandSpec(req: ApplyPatchRequest): Result<CommandSpec> {
        val exe = req.codexExe ?: getCurrentExe()
            .getOrElse { return Result.failure(ToolError.Rejected("failed to determine codex exe: ${it.message}")) }
        
        return Result.success(CommandSpec(
            program = exe,
            args = listOf(CODEX_APPLY_PATCH_ARG1, req.patch),
            cwd = req.cwd,
            expiration = if (req.timeoutMs != null) ExecExpiration.Timeout(req.timeoutMs.milliseconds) else ExecExpiration.DefaultTimeout,
            env = emptyMap(), // Minimal environment
            withEscalatedPermissions = null,
            justification = null
        ))
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

    private fun getCurrentExe(): Result<String> {
        // TODO: Implement platform specific current exe retrieval
        // For now return a placeholder or throw
        return Result.success("codex") 
    }
}
