```
// port-lint: source core/src/tools/runtimes/unified_exec.rs
package ai.solace.coder.core.tools.runtimes

import ai.solace.coder.core.session.Session as CodexSession
import ai.solace.coder.core.session.TurnContext
import ai.solace.coder.core.session.SessionServices
import ai.solace.coder.core.tools.Approvable
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
import ai.solace.coder.core.unified_exec.UnifiedExecSessionManager
import ai.solace.coder.core.unified_exec.UnifiedExecSession
import ai.solace.coder.core.unified_exec.UnifiedExecError
import ai.solace.coder.protocol.ReviewDecision
import ai.solace.coder.core.ExecExpiration
import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.unified_exec.Errors.SandboxError
import ai.solace.coder.core.tools.buildCommandSpec

data class UnifiedExecRequest(
    val command: List<String>,
    val cwd: String,
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

data class UnifiedExecApprovalKey(
    val command: List<String>,
    val cwd: String,
    val escalated: Boolean
)

class UnifiedExecRuntime(
    private val manager: UnifiedExecSessionManager
) : ToolRuntime<UnifiedExecRequest, UnifiedExecSession>, Sandboxable, Approvable<UnifiedExecRequest> {

    override fun sandboxPreference(): SandboxablePreference {
        return SandboxablePreference.Auto
    }

    override fun escalateOnFailure(): Boolean {
        return true
    }

    // Not overriding because it's not in the interface (commented out in Sandboxing.kt)
    fun approvalKey(req: UnifiedExecRequest): Any {
        return UnifiedExecApprovalKey(
            command = req.command,
            cwd = req.cwd,
            escalated = req.withEscalatedPermissions ?: false
        )
    }

    override suspend fun startApprovalAsync(req: UnifiedExecRequest, ctx: ApprovalCtx): ReviewDecision {
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

    override fun approvalRequirement(req: UnifiedExecRequest): ApprovalRequirement? {
        return req.approvalRequirement
    }

    override fun sandboxModeForFirstAttempt(req: UnifiedExecRequest): SandboxOverride {
        val bypass = req.withEscalatedPermissions == true || 
                     (req.approvalRequirement is ApprovalRequirement.Skip && req.approvalRequirement.bypassSandbox)
        
        return if (bypass) {
            SandboxOverride.BypassSandboxFirstAttempt
        } else {
            SandboxOverride.NoOverride
        }
    }

    override suspend fun run(
        req: UnifiedExecRequest,
        attempt: SandboxAttempt,
        ctx: ToolCtx
    ): Result<UnifiedExecSession> {
        val specResult = buildCommandSpec(
            req.command,
            req.cwd,
            req.env,
            ExecExpiration.DefaultTimeout,
            req.withEscalatedPermissions,
            req.justification
        )
        val spec = specResult.getOrElse { return Result.failure(ToolError.Rejected("missing command line for PTY")) }

        val execEnvResult = attempt.envFor(spec)
        val execEnv = execEnvResult.getOrElse { return Result.failure(ToolError.Codex(it.message ?: "Unknown error")) }

        return try {
            val session = manager.openSessionWithExecEnv(execEnv) // This method needs to be added to SessionManager
            Result.success(session)
        } catch (e: Exception) {
            // Map UnifiedExecError to ToolError
            // In Rust: UnifiedExecError::SandboxDenied -> ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied))
            // Here we catch Exception, check type if possible
            if (e is UnifiedExecError.SandboxDenied) {
                Result.failure(ToolError.Codex(CodexError.Sandbox(SandboxError(e.message))))
            } else {
                Result.failure(ToolError.Rejected(e.message ?: "Unknown error"))
            }
        }
    }
}
```
