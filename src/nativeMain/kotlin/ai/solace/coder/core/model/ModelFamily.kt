package ai.solace.coder.core.model

import ai.solace.coder.protocol.ReasoningEffortConfig

data class ModelFamily(
    val effectiveContextWindowPercent: Int,
    val supportsReasoningSummaries: Boolean,
    val defaultReasoningEffort: ReasoningEffortConfig?,
    val supportVerbosity: Boolean,
    val defaultVerbosity: Int?,
    val contextWindow: Long,
    val autoCompactTokenLimit: Long?
)
