// port-lint: source core/src/tools/runtimes/apply_patch.rs
package ai.solace.coder.core.tools.runtimes

import ai.solace.coder.core.tools.Approvable
import ai.solace.coder.core.tools.ApprovalCtx
import ai.solace.coder.core.tools.ApprovalRequirement
import ai.solace.coder.core.tools.SandboxAttempt
import ai.solace.coder.core.tools.Sandboxable
import ai.solace.coder.core.tools.SandboxablePreference
import ai.solace.coder.core.tools.ToolCtx
import ai.solace.coder.core.tools.ToolError
import ai.solace.coder.core.tools.ToolRuntime
import ai.solace.coder.protocol.ReviewDecision

class ApplyPatchRuntime : ToolRuntime<Any, Any> {
    override fun sandboxPreference(): SandboxablePreference {
        return SandboxablePreference.Forbid
    }

    override suspend fun startApprovalAsync(req: Any, ctx: ApprovalCtx): ReviewDecision {
        return ReviewDecision.Approved
    }

    override suspend fun run(req: Any, attempt: SandboxAttempt, ctx: ToolCtx): Result<Any> {
        return Result.success(Unit)
    }
}
