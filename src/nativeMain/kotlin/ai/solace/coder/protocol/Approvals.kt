// port-lint: source protocol/src/approvals.rs
package ai.solace.coder.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Approval-related types.
 *
 * Ported from Rust codex-rs/protocol/src/approvals.rs
 */

@Serializable
enum class SandboxRiskLevel {
    @SerialName("low")
    Low,

    @SerialName("medium")
    Medium,

    @SerialName("high")
    High;

    fun asStr(): String = when (this) {
        Low -> "low"
        Medium -> "medium"
        High -> "high"
    }
}

@Serializable
data class SandboxCommandAssessment(
    val description: String,
    @SerialName("risk_level")
    val riskLevel: SandboxRiskLevel
)

@Serializable
data class ExecApprovalRequestEvent(
    @SerialName("call_id")
    val callId: String,
    @SerialName("turn_id")
    val turnId: String = "",
    val command: List<String>,
    val cwd: String,
    val reason: String? = null,
    val risk: SandboxCommandAssessment? = null,
    @SerialName("parsed_cmd")
    val parsedCmd: List<ParsedCommand>
)

@Serializable
data class ElicitationRequestEvent(
    @SerialName("server_name")
    val serverName: String,
    val id: String,
    val message: String
)

@Serializable
enum class ElicitationAction {
    @SerialName("accept")
    Accept,

    @SerialName("decline")
    Decline,

    @SerialName("cancel")
    Cancel
}

@Serializable
data class ApplyPatchApprovalRequestEvent(
    @SerialName("call_id")
    val callId: String,
    @SerialName("turn_id")
    val turnId: String = "",
    val changes: Map<String, FileChange>,
    val reason: String? = null,
    @SerialName("grant_root")
    val grantRoot: String? = null
)
