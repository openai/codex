// port-lint: source codex-rs/core/src/tools/sandboxing.rs
package ai.solace.coder.exec.sandbox

import ai.solace.coder.protocol.ReviewDecision
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/**
 * Store for caching approval decisions across tool calls.
 * Mirrors Rust's ApprovalStore from tools/sandboxing.rs
 */
class ApprovalStore {
    @PublishedApi
    internal val map = mutableMapOf<String, ReviewDecision>()

    /**
     * Get a cached approval decision for a key.
     * @param key The serializable key (e.g., command hash, tool call ID)
     * @return The cached decision, or null if not found
     */
    inline fun <reified K> get(key: K): ReviewDecision? where K : @Serializable Any {
        val serialized = try {
            Json.encodeToString(key)
        } catch (e: Exception) {
            return null
        }
        return map[serialized]
    }

    /**
     * Store an approval decision for a key.
     * @param key The serializable key
     * @param value The approval decision to cache
     */
    inline fun <reified K> put(key: K, value: ReviewDecision) where K : @Serializable Any {
        val serialized = try {
            Json.encodeToString(key)
        } catch (e: Exception) {
            return
        }
        map[serialized] = value
    }
}

/**
 * Specifies what the tool orchestrator should do with a given tool call.
 * Mirrors Rust's ApprovalRequirement enum from tools/sandboxing.rs
 */
sealed class ApprovalRequirement {
    /**
     * No approval required for this tool call.
     * @param bypassSandbox If true, the first attempt should skip sandboxing
     */
    data class Skip(val bypassSandbox: Boolean) : ApprovalRequirement()

    /**
     * Approval required for this tool call.
     * @param reason Optional explanation for why approval is needed
     */
    data class NeedsApproval(val reason: String?) : ApprovalRequirement()

    /**
     * Execution forbidden for this tool call.
     * @param reason Explanation for why execution is forbidden
     */
    data class Forbidden(val reason: String) : ApprovalRequirement()
}

/**
 * Assessment of whether a command is safe to execute in a sandbox.
 * Mirrors Rust's SandboxCommandAssessment from protocol.
 */
data class SandboxCommandAssessment(
    val safe: Boolean,
    val reason: String?
)

/**
 * Tool error types.
 * Mirrors Rust's ToolError enum from tools/sandboxing.rs
 */
sealed class ToolError {
    data class Rejected(val message: String) : ToolError()
    data class Codex(val error: Exception) : ToolError()
}

/**
 * Captures command metadata needed to re-run a tool request without sandboxing.
 * Mirrors Rust's SandboxRetryData from tools/sandboxing.rs
 */
data class SandboxRetryData(
    val command: List<String>,
    val cwd: String
)

