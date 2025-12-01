// port-lint: source protocol/src/plan_tool.rs
package ai.solace.coder.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Types for the TODO/plan tool arguments.
 *
 * Ported from Rust codex-rs/protocol/src/plan_tool.rs
 */

@Serializable
enum class StepStatus {
    @SerialName("pending")
    Pending,

    @SerialName("in_progress")
    InProgress,

    @SerialName("completed")
    Completed
}

@Serializable
data class PlanItemArg(
    val step: String,
    val status: StepStatus
)

@Serializable
data class UpdatePlanArgs(
    val explanation: String? = null,
    val plan: List<PlanItemArg>
)
