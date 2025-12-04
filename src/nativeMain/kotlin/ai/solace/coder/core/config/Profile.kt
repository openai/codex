package ai.solace.coder.core.config

import ai.solace.coder.protocol.AskForApproval
import ai.solace.coder.protocol.ReasoningEffort
import ai.solace.coder.protocol.ReasoningSummary
import ai.solace.coder.protocol.SandboxMode
import ai.solace.coder.protocol.Verbosity
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class ConfigProfile(
    @SerialName("model") val model: String? = null,
    @SerialName("model_provider") val modelProvider: String? = null,
    @SerialName("approval_policy") val approvalPolicy: AskForApproval? = null,
    @SerialName("sandbox_mode") val sandboxMode: SandboxMode? = null,
    @SerialName("model_reasoning_effort") val modelReasoningEffort: ReasoningEffort? = null,
    @SerialName("model_reasoning_summary") val modelReasoningSummary: ReasoningSummary? = null,
    @SerialName("model_verbosity") val modelVerbosity: Verbosity? = null,
    @SerialName("chatgpt_base_url") val chatgptBaseUrl: String? = null,
    @SerialName("experimental_instructions_file") val experimentalInstructionsFile: String? = null,
    @SerialName("experimental_compact_prompt_file") val experimentalCompactPromptFile: String? = null,
    @SerialName("include_apply_patch_tool") val includeApplyPatchTool: Boolean? = null,
    @SerialName("experimental_use_unified_exec_tool") val experimentalUseUnifiedExecTool: Boolean? = null,
    @SerialName("experimental_use_rmcp_client") val experimentalUseRmcpClient: Boolean? = null,
    @SerialName("experimental_use_freeform_apply_patch") val experimentalUseFreeformApplyPatch: Boolean? = null,
    @SerialName("experimental_sandbox_command_assessment") val experimentalSandboxCommandAssessment: Boolean? = null,
    @SerialName("tools_web_search") val toolsWebSearch: Boolean? = null,
    @SerialName("tools_view_image") val toolsViewImage: Boolean? = null,
    @SerialName("features") val features: FeaturesToml? = null,
    @SerialName("oss_provider") val ossProvider: String? = null,
)
