package ai.solace.coder.core.config

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Direct deserialization target for config files (TOML/JSON). Optional fields only.
@Serializable
data class ConfigToml(
    @SerialName("profile") val profile: String? = null,
    @SerialName("profiles") val profiles: Map<String, ConfigProfile>? = null,

    @SerialName("model") val model: String? = null,
    @SerialName("model_provider") val modelProvider: String? = null,

    // MCP servers keyed by name; Raw form will be normalized later.
    @SerialName("mcp_servers") val mcpServers: Map<String, RawMcpServerConfig>? = null,
)

// Runtime overrides such as CLI flags or env variables.
@Serializable
data class ConfigOverrides(
    @SerialName("profile") val profile: String? = null,
    @SerialName("model") val model: String? = null,
    @SerialName("model_provider") val modelProvider: String? = null,
)
