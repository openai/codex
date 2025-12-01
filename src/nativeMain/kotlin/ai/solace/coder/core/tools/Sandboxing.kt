// port-lint: source core/src/tools/sandboxing.rs
package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.session.Session
import ai.solace.coder.core.session.TurnContext
import ai.solace.coder.exec.sandbox.CommandSpec
import ai.solace.coder.exec.sandbox.ExecEnv
import ai.solace.coder.exec.sandbox.SandboxManager
import ai.solace.coder.exec.sandbox.SandboxTransformError
import ai.solace.coder.exec.sandbox.SandboxType
import ai.solace.coder.protocol.AskForApproval
import ai.solace.coder.protocol.ReviewDecision
import ai.solace.coder.protocol.SandboxCommandAssessment
import ai.solace.coder.protocol.SandboxPolicy
import kotlinx.coroutines.sync.Mutex
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

class ApprovalStore {
    private val map = mutableMapOf<String, ReviewDecision>()

    fun <K> get(key: K): ReviewDecision? where K : Any {
        // Assuming K is serializable or we have a way to serialize it
        // For now using toString() as a placeholder if not strictly serializable in Kotlin Native context easily without reflection
        // Ideally we should use kotlinx.serialization
        val s = try {
            // This requires K to be marked @Serializable and using a serializer, which is hard with generics in Kotlin
            // We might need to pass a serializer or restrict K
            // For now, let's assume key.toString() is unique enough or use a specific interface
            key.toString() 
        } catch (e: Exception) {
            return null
        }
        return map[s]
    }

    fun <K> put(key: K, value: ReviewDecision) where K : Any {
        val s = key.toString()
        map[s] = value
    }
}

suspend fun <K> withCachedApproval(
    services: SessionServices, // Placeholder for SessionServices
    key: K,
    fetch: suspend () -> ReviewDecision
): ReviewDecision where K : Any {
    val store = services.toolApprovals
    // lock logic
    // store.get(key)?.let { return it }
    
    val decision = fetch()
    
    if (decision == ReviewDecision.ApprovedForSession) {
        // store.put(key, decision)
    }
    
    return decision
}

// Placeholder for SessionServices
class SessionServices {
    val toolApprovals = ApprovalStore() // Should be Mutex protected
}

data class ApprovalCtx(
    val session: Session,
    val turn: TurnContext,
    val callId: String,
    val retryReason: String?,
    val risk: SandboxCommandAssessment?
)

sealed class ApprovalRequirement {
    data class Skip(val bypassSandbox: Boolean) : ApprovalRequirement()
    data class NeedsApproval(val reason: String?) : ApprovalRequirement()
    data class Forbidden(val reason: String) : ApprovalRequirement()
}

fun defaultApprovalRequirement(
    policy: AskForApproval,
    sandboxPolicy: SandboxPolicy
): ApprovalRequirement {
    val needsApproval = when (policy) {
        AskForApproval.Never, AskForApproval.OnFailure -> false
        AskForApproval.OnRequest -> sandboxPolicy != SandboxPolicy.DangerFullAccess
        AskForApproval.UnlessTrusted -> true
    }

    return if (needsApproval) {
        ApprovalRequirement.NeedsApproval(null)
    } else {
        ApprovalRequirement.Skip(bypassSandbox = false)
    }
}

enum class SandboxOverride {
    NoOverride,
    BypassSandboxFirstAttempt
}

interface Approvable<Req> {
    // type ApprovalKey
    // fun approvalKey(req: Req): ApprovalKey

    fun sandboxModeForFirstAttempt(req: Req): SandboxOverride {
        return SandboxOverride.NoOverride
    }

    fun shouldBypassApproval(policy: AskForApproval, alreadyApproved: Boolean): Boolean {
        if (alreadyApproved) {
            return true
        }
        return policy == AskForApproval.Never
    }

    fun approvalRequirement(req: Req): ApprovalRequirement? {
        return null
    }

    fun wantsNoSandboxApproval(policy: AskForApproval): Boolean {
        return policy != AskForApproval.Never && policy != AskForApproval.OnRequest
    }

    suspend fun startApprovalAsync(req: Req, ctx: ApprovalCtx): ReviewDecision
}

enum class SandboxablePreference {
    Auto,
    Require,
    Forbid
}

interface Sandboxable {
    fun sandboxPreference(): SandboxablePreference
    fun escalateOnFailure(): Boolean {
        return true
    }
}

data class ToolCtx(
    val session: Session,
    val turn: TurnContext,
    val callId: String,
    val toolName: String
)

data class SandboxRetryData(
    val command: List<String>,
    val cwd: String // PathBuf -> String
)

interface ProvidesSandboxRetryData {
    fun sandboxRetryData(): SandboxRetryData?
}

sealed class ToolError {
    data class Rejected(val reason: String) : ToolError()
    data class Codex(val error: CodexError) : ToolError()
}

interface ToolRuntime<Req, Out> : Approvable<Req>, Sandboxable {
    suspend fun run(
        req: Req,
        attempt: SandboxAttempt,
        ctx: ToolCtx
    ): Result<Out> // Using Result<Out> which can wrap ToolError logic or throw
}

class SandboxAttempt(
    val sandbox: SandboxType,
    val policy: SandboxPolicy,
    val manager: SandboxManager,
    val sandboxCwd: String, // Path -> String
    val codexLinuxSandboxExe: String? // PathBuf -> String
) {
    fun envFor(spec: CommandSpec): Result<ExecEnv> {
        return manager.transform(
            spec,
            policy,
            sandbox,
            sandboxCwd,
            codexLinuxSandboxExe
        )
    }
}
