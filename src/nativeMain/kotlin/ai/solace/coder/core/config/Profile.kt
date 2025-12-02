package ai.solace.coder.core.config

import ai.solace.coder.protocol.AskForApproval
import ai.solace.coder.protocol.ReasoningEffort
import ai.solace.coder.protocol.ReasoningSummary
import ai.solace.coder.protocol.SandboxMode
import ai.solace.coder.protocol.Verbosity
import kotlinx.serialization.Serializable

@Serializable
data class ConfigProfile(
    val model: String? = null,
    val modelProvider: String? = null,
    val approvalPolicy: AskForApproval? = null,
    val sandboxMode: SandboxMode? = null,
    val modelReasoningEffort: ReasoningEffort? = null,
    val modelReasoningSummary: ReasoningSummary? = null,
    val modelVerbosity: Verbosity? = null,
    val chatgptBaseUrl: String? = null,
    val experimentalInstructionsFile: String? = null,
    val experimentalCompactPromptFile: String? = null,
    val includeApplyPatchTool: Boolean? = null,
    val experimentalUseUnifiedExecTool: Boolean? = null,
    val experimentalUseRmcpClient: Boolean? = null,
    val experimentalUseFreeformApplyPatch: Boolean? = null,
    val experimentalSandboxCommandAssessment: Boolean? = null,
    val toolsWebSearch: Boolean? = null,
    val toolsViewImage: Boolean? = null,
    val features: FeaturesToml? = null,
    val ossProvider: String? = null,
)
