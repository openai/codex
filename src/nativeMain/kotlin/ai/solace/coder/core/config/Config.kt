package ai.solace.coder.core.config

import ai.solace.coder.core.auth.AuthCredentialsStoreMode
import ai.solace.coder.core.ForcedLoginMethod
import ai.solace.coder.core.model.ModelFamily
import kotlinx.io.files.Path
import kotlinx.serialization.json.JsonElement

data class Config(
    val codexHome: Path,
    val cliAuthCredentialsStoreMode: AuthCredentialsStoreMode,
    val forcedLoginMethod: ForcedLoginMethod? = null,
    val forcedChatgptWorkspaceId: String? = null,
    val model: String,
    val modelFamily: ModelFamily,
    val modelContextWindow: Long? = null,
    val modelAutoCompactTokenLimit: Long? = null,
    val modelVerbosity: Int? = null,
    val showRawAgentReasoning: Boolean = false,
    val outputSchema: JsonElement? = null,
    val tools: List<Any> = emptyList()
)
