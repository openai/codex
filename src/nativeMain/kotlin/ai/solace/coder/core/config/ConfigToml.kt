package ai.solace.coder.core.config

import kotlinx.serialization.Serializable

// Direct deserialization target for config files (TOML/JSON). Optional fields only.
@Serializable
data class ConfigToml(
    val profile: String? = null,
    val profiles: Map<String, ConfigProfile>? = null,

    val model: String? = null,
    val modelProvider: String? = null,

    // MCP servers keyed by name; Raw form will be normalized later.
    val mcpServers: Map<String, RawMcpServerConfig>? = null,
)

// Runtime overrides such as CLI flags or env variables.
@Serializable
data class ConfigOverrides(
    val profile: String? = null,
    val model: String? = null,
    val modelProvider: String? = null,
)
