package ai.solace.coder.core.config

/**
 * Loader for Codex configuration. This is a first-pass skeleton that will be
 * expanded to mirror Rust `mod.rs` behavior (defaults → file → profile → overrides → env).
 */
data class LoadedConfig(
    val model: String,
    val modelProvider: String?,
    val mcpServers: Map<String, McpServerConfig>,
    val otelEnvironment: String = DEFAULT_OTEL_ENVIRONMENT,
)

object ConfigLoader {
    /**
     * Build a minimal Config from provided inputs. For now, this does not read files.
     * Later iterations will add TOML parsing and full precedence rules.
     */
    fun load(
        base: ConfigToml? = null,
        selectedProfile: String? = null,
        overrides: ConfigOverrides? = null,
    ): Result<LoadedConfig> {
        // 1) Start from base (file) values
        val fileModel = base?.model
        val fileProvider = base?.modelProvider

        // 2) Determine active profile (by argument or from file)
        val profileName = overrides?.profile ?: selectedProfile ?: base?.profile
        val profile = base?.profiles?.get(profileName ?: "")

        // 3) Apply overrides (last)
        val model = overrides?.model ?: profile?.model ?: fileModel ?: "gpt-4o-mini"
        val modelProvider = overrides?.modelProvider ?: profile?.modelProvider ?: fileProvider

        // 4) Normalize MCP servers if present
        val mcp: Map<String, McpServerConfig> = base?.mcpServers?.mapValues { (_, raw) ->
            raw.normalize()
        } ?: emptyMap()

        return Result.success(
            LoadedConfig(
                model = model,
                modelProvider = modelProvider,
                mcpServers = mcp,
                otelEnvironment = DEFAULT_OTEL_ENVIRONMENT,
            )
        )
    }
}
