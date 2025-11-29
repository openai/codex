package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.core.session.Session
import ai.solace.coder.core.session.TurnContext
import ai.solace.coder.exec.process.SandboxType
import ai.solace.coder.exec.sandbox.SandboxManager
import ai.solace.coder.exec.sandbox.SandboxPreference
import ai.solace.coder.protocol.AskForApproval
import ai.solace.coder.protocol.ReviewDecision
import ai.solace.coder.protocol.SandboxPolicy as ProtocolSandboxPolicy
import ai.solace.coder.protocol.models.SandboxPolicy as ModelsSandboxPolicy

/**
 * Central place for approvals + sandbox selection + retry semantics.
 * Drives a simple sequence for any ToolRuntime:
 * approval → select sandbox → attempt → retry without sandbox on denial (no re‑approval thanks to caching).
 *
 * Ported from Rust codex-rs/core/src/tools/orchestrator.rs
 */
class ToolOrchestrator {
    private val sandbox = SandboxManager()

    /**
     * Run a tool with approval workflow and sandbox retry logic.
     *
     * @param tool The tool runtime to execute
     * @param request The tool request
     * @param toolCtx Context for the tool invocation
     * @param turnCtx Turn context with sandbox policy
     * @param approvalPolicy The approval policy to use
     */
    suspend fun <Req, Out> run(
        tool: ToolRuntime<Req, Out>,
        request: Req,
        toolCtx: ToolContext,
        turnCtx: TurnContext,
        approvalPolicy: AskForApproval
    ): CodexResult<Out> where Req : ProvidesSandboxRetryData {
        var alreadyApproved = false

        // 1) Approval
        val requirement = tool.approvalRequirement(request)
            ?: defaultApprovalRequirement(approvalPolicy, turnCtx.sandboxPolicy)

        when (requirement) {
            is ApprovalRequirement.Skip -> {
                // Auto-approved by config
            }
            is ApprovalRequirement.Forbidden -> {
                return CodexResult.failure(
                    CodexError.Fatal("Tool rejected: ${requirement.reason}")
                )
            }
            is ApprovalRequirement.NeedsApproval -> {
                val approvalCtx = ApprovalContext(
                    session = toolCtx.session,
                    turn = turnCtx,
                    callId = toolCtx.callId,
                    retryReason = requirement.reason,
                    risk = null
                )
                val decision = tool.startApprovalAsync(request, approvalCtx)

                when (decision) {
                    ReviewDecision.Denied, ReviewDecision.Abort -> {
                        return CodexResult.failure(
                            CodexError.Fatal("rejected by user")
                        )
                    }
                    ReviewDecision.Approved, ReviewDecision.ApprovedForSession -> {
                        // Approved, continue
                    }
                }
                alreadyApproved = true
            }
        }

        // 2) First attempt under the selected sandbox
        val modelsSandboxPolicy = turnCtx.sandboxPolicy.toModelsPolicy()
        val initialSandbox = when (tool.sandboxModeForFirstAttempt(request)) {
            SandboxOverride.BypassSandboxFirstAttempt -> SandboxType.None
            SandboxOverride.NoOverride -> sandbox.selectInitialSandbox(
                modelsSandboxPolicy,
                tool.sandboxPreference()
            )
        }

        val initialAttempt = SandboxAttempt(
            sandbox = initialSandbox,
            policy = modelsSandboxPolicy,
            manager = sandbox,
            sandboxCwd = turnCtx.cwd,
            codexLinuxSandboxExe = null
        )

        val initialResult = tool.run(request, initialAttempt, toolCtx)

        return when {
            initialResult.isSuccess() -> initialResult
            isSandboxDeniedError(initialResult) -> {
                handleSandboxDenial(
                    tool, request, toolCtx, turnCtx,
                    approvalPolicy, alreadyApproved, initialResult
                )
            }
            else -> initialResult
        }
    }

    /**
     * Handle sandbox denial by optionally retrying without sandbox.
     */
    private suspend fun <Req, Out> handleSandboxDenial(
        tool: ToolRuntime<Req, Out>,
        request: Req,
        toolCtx: ToolContext,
        turnCtx: TurnContext,
        approvalPolicy: AskForApproval,
        alreadyApproved: Boolean,
        originalResult: CodexResult<Out>
    ): CodexResult<Out> where Req : ProvidesSandboxRetryData {
        // Check if tool wants to escalate on failure
        if (!tool.escalateOnFailure()) {
            return originalResult
        }

        // Under Never or OnRequest, do not retry without sandbox
        if (!tool.wantsNoSandboxApproval(approvalPolicy)) {
            return originalResult
        }

        // Ask for approval before retrying without sandbox
        if (!tool.shouldBypassApproval(approvalPolicy, alreadyApproved)) {
            val approvalCtx = ApprovalContext(
                session = toolCtx.session,
                turn = turnCtx,
                callId = toolCtx.callId,
                retryReason = "command failed; retry without sandbox?",
                risk = null
            )

            val decision = tool.startApprovalAsync(request, approvalCtx)

            when (decision) {
                ReviewDecision.Denied, ReviewDecision.Abort -> {
                    return CodexResult.failure(
                        CodexError.Fatal("rejected by user")
                    )
                }
                ReviewDecision.Approved, ReviewDecision.ApprovedForSession -> {
                    // Continue with retry
                }
            }
        }

        // Second attempt without sandbox
        val escalatedAttempt = SandboxAttempt(
            sandbox = SandboxType.None,
            policy = turnCtx.sandboxPolicy.toModelsPolicy(),
            manager = sandbox,
            sandboxCwd = turnCtx.cwd,
            codexLinuxSandboxExe = null
        )

        return tool.run(request, escalatedAttempt, toolCtx)
    }

    /**
     * Check if the error is a sandbox denial.
     */
    private fun <Out> isSandboxDeniedError(result: CodexResult<Out>): Boolean {
        if (result.isSuccess()) return false
        val error = (result as? CodexResult.Failure)?.error ?: return false
        return error is CodexError.SandboxError
    }

    /**
     * Default approval requirement based on policy.
     */
    private fun defaultApprovalRequirement(
        approvalPolicy: AskForApproval,
        sandboxPolicy: ProtocolSandboxPolicy
    ): ApprovalRequirement {
        // AskForApproval enum values: UnlessTrusted, OnFailure, OnRequest, Never
        return when (approvalPolicy) {
            AskForApproval.Never -> ApprovalRequirement.Skip()
            AskForApproval.OnRequest -> ApprovalRequirement.Skip()
            AskForApproval.OnFailure -> ApprovalRequirement.Skip()
            AskForApproval.UnlessTrusted -> {
                when (sandboxPolicy) {
                    is ProtocolSandboxPolicy.DangerFullAccess ->
                        ApprovalRequirement.NeedsApproval("full disk access enabled")
                    else -> ApprovalRequirement.Skip()
                }
            }
        }
    }
}

/**
 * Convert Protocol SandboxPolicy to Models SandboxPolicy.
 */
private fun ProtocolSandboxPolicy.toModelsPolicy(): ModelsSandboxPolicy {
    return when (this) {
        is ProtocolSandboxPolicy.DangerFullAccess -> ModelsSandboxPolicy.DangerFullAccess
        is ProtocolSandboxPolicy.ReadOnly -> ModelsSandboxPolicy.ReadOnly(
            readablePaths = emptyList(),
            networkAccess = false
        )
        is ProtocolSandboxPolicy.WorkspaceWrite -> ModelsSandboxPolicy.WorkspaceWrite(
            writableRoots = this.writableRoots,
            networkAccess = this.networkAccess,
            excludeTmpdirEnvVar = this.excludeTmpdirEnvVar,
            excludeSlashTmp = this.excludeSlashTmp
        )
    }
}

/**
 * Context for a tool invocation.
 */
data class ToolContext(
    val session: Session,
    val toolName: String,
    val callId: String
)

/**
 * Context for approval requests.
 */
data class ApprovalContext(
    val session: Session,
    val turn: TurnContext,
    val callId: String,
    val retryReason: String?,
    val risk: String?
)

/**
 * Approval requirement for a tool invocation.
 */
sealed class ApprovalRequirement {
    data class Skip(val reason: String? = null) : ApprovalRequirement()
    data class Forbidden(val reason: String) : ApprovalRequirement()
    data class NeedsApproval(val reason: String?) : ApprovalRequirement()
}

/**
 * Override for sandbox behavior on first attempt.
 */
enum class SandboxOverride {
    NoOverride,
    BypassSandboxFirstAttempt
}

/**
 * Sandbox attempt configuration.
 */
data class SandboxAttempt(
    val sandbox: SandboxType,
    val policy: ModelsSandboxPolicy,
    val manager: SandboxManager,
    val sandboxCwd: String,
    val codexLinuxSandboxExe: String?
)

/**
 * Interface for data that can provide sandbox retry metadata.
 */
interface ProvidesSandboxRetryData {
    fun sandboxRetryData(): SandboxRetryMetadata?
}

/**
 * Metadata for sandbox retry decisions.
 */
data class SandboxRetryMetadata(
    val command: List<String>
)

/**
 * Runtime interface for tool execution.
 */
interface ToolRuntime<Req, Out> {
    /**
     * Get the approval requirement for this request.
     */
    fun approvalRequirement(request: Req): ApprovalRequirement?

    /**
     * Get the sandbox preference for this tool.
     */
    fun sandboxPreference(): SandboxPreference = SandboxPreference.Auto

    /**
     * Get the sandbox mode override for the first attempt.
     */
    fun sandboxModeForFirstAttempt(request: Req): SandboxOverride = SandboxOverride.NoOverride

    /**
     * Whether to escalate (retry without sandbox) on failure.
     */
    fun escalateOnFailure(): Boolean = true

    /**
     * Whether this tool wants no-sandbox approval under the given policy.
     */
    fun wantsNoSandboxApproval(policy: AskForApproval): Boolean {
        return policy != AskForApproval.Never && policy != AskForApproval.OnRequest
    }

    /**
     * Whether to bypass approval for retry.
     */
    fun shouldBypassApproval(policy: AskForApproval, alreadyApproved: Boolean): Boolean {
        // In the original Rust code this was "Always", but our enum uses "UnlessTrusted"
        // which is the strictest approval mode
        return alreadyApproved && policy != AskForApproval.UnlessTrusted
    }

    /**
     * Start an approval request asynchronously.
     */
    suspend fun startApprovalAsync(request: Req, ctx: ApprovalContext): ReviewDecision

    /**
     * Run the tool with the given sandbox configuration.
     */
    suspend fun run(
        request: Req,
        attempt: SandboxAttempt,
        ctx: ToolContext
    ): CodexResult<Out>
}
