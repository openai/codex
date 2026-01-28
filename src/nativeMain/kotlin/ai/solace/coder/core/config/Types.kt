package ai.solace.coder.core.config

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlin.time.Duration

const val DEFAULT_OTEL_ENVIRONMENT: String = "dev"

@Serializable
sealed class McpServerTransportConfig {
    @Serializable
    data class Stdio(
        @SerialName("command") val command: String,
        @SerialName("args") val args: List<String> = emptyList(),
        @SerialName("env") val env: Map<String, String>? = null,
        @SerialName("env_vars") val envVars: List<String>? = null,
        @SerialName("cwd") val cwd: String? = null,
        @SerialName("http_headers") val httpHeaders: Map<String, String>? = null,
        @SerialName("env_http_headers") val envHttpHeaders: Map<String, String>? = null,
    ) : McpServerTransportConfig()

    @Serializable
    data class StreamableHttp(
        @SerialName("url") val url: String,
        @SerialName("bearer_token") val bearerToken: String? = null,
        @SerialName("bearer_token_env_var") val bearerTokenEnvVar: String? = null,
    ) : McpServerTransportConfig()
}

@Serializable
data class McpServerConfig(
    @SerialName("transport") val transport: McpServerTransportConfig,
    @SerialName("enabled") val enabled: Boolean = true,
    // Note: these are normalized fields; serialization names retained for parity in raw
    @SerialName("startup_timeout_sec") val startupTimeout: Duration? = null,
    @SerialName("tool_timeout_sec") val toolTimeout: Duration? = null,
    @SerialName("enabled_tools") val enabledTools: List<String>? = null,
    @SerialName("disabled_tools") val disabledTools: List<String>? = null,
)

@Serializable
data class RawMcpServerConfig(
    // stdio
    @SerialName("command") val command: String? = null,
    @SerialName("args") val args: List<String>? = null,
    @SerialName("env") val env: Map<String, String>? = null,
    @SerialName("env_vars") val envVars: List<String>? = null,
    @SerialName("cwd") val cwd: String? = null,
    @SerialName("http_headers") val httpHeaders: Map<String, String>? = null,
    @SerialName("env_http_headers") val envHttpHeaders: Map<String, String>? = null,
    // streamable_http
    @SerialName("url") val url: String? = null,
    @SerialName("bearer_token") val bearerToken: String? = null,
    @SerialName("bearer_token_env_var") val bearerTokenEnvVar: String? = null,
    // shared
    @SerialName("startup_timeout_sec") val startupTimeoutSec: Double? = null,
    @SerialName("startup_timeout_ms") val startupTimeoutMs: Long? = null,
    @SerialName("tool_timeout_sec") val toolTimeoutSec: Double? = null,
    @SerialName("enabled") val enabled: Boolean? = null,
    @SerialName("enabled_tools") val enabledTools: List<String>? = null,
    @SerialName("disabled_tools") val disabledTools: List<String>? = null,
) {
    fun normalize(): McpServerConfig {
        val startup = secondsToDuration(startupTimeoutSec) ?: millisToDuration(startupTimeoutMs)
        val tool = secondsToDuration(toolTimeoutSec)

        val isStdio = command != null || args != null || env != null || envVars != null || cwd != null || httpHeaders != null || envHttpHeaders != null
        val isHttp = url != null || bearerToken != null || bearerTokenEnvVar != null

        val transport = when {
            isStdio && isHttp -> error("invalid MCP server config: mix of stdio and streamable_http fields")
            isHttp -> McpServerTransportConfig.StreamableHttp(
                url = url ?: error("invalid MCP server config: missing 'url' for streamable_http"),
                bearerToken = bearerToken,
                bearerTokenEnvVar = bearerTokenEnvVar,
            )
            isStdio -> McpServerTransportConfig.Stdio(
                command = command ?: error("invalid MCP server config: missing 'command' for stdio"),
                args = args ?: emptyList(),
                env = env,
                envVars = envVars,
                cwd = cwd,
                httpHeaders = httpHeaders,
                envHttpHeaders = envHttpHeaders,
            )
            else -> error("invalid MCP server config: missing transport fields (need either 'url' or 'command')")
        }

        return McpServerConfig(
            transport = transport,
            enabled = enabled ?: true,
            startupTimeout = startup,
            toolTimeout = tool,
            enabledTools = enabledTools,
            disabledTools = disabledTools,
        )
    }
}

@Serializable
data class FeaturesToml(
    val entries: Map<String, Boolean> = emptyMap()
)

// ============================================================================
// URI-based file opener
// ============================================================================

@Serializable
enum class UriBasedFileOpener {
    @SerialName("vscode")
    VsCode,

    @SerialName("vscode-insiders")
    VsCodeInsiders,

    @SerialName("windsurf")
    Windsurf,

    @SerialName("cursor")
    Cursor,

    @SerialName("none")
    None;

    fun getScheme(): String? = when (this) {
        VsCode -> "vscode"
        VsCodeInsiders -> "vscode-insiders"
        Windsurf -> "windsurf"
        Cursor -> "cursor"
        None -> null
    }
}

// ============================================================================
// History settings
// ============================================================================

@Serializable
enum class HistoryPersistence {
    @SerialName("save-all")
    SaveAll,

    @SerialName("none")
    None
}

@Serializable
data class History(
    val persistence: HistoryPersistence = HistoryPersistence.SaveAll,
    @SerialName("max_bytes")
    val maxBytes: Long? = null
)

// ============================================================================
// OTEL configuration
// ============================================================================

@Serializable
enum class OtelHttpProtocol {
    @SerialName("binary")
    Binary,

    @SerialName("json")
    Json
}

@Serializable
data class OtelTlsConfig(
    @SerialName("ca_certificate")
    val caCertificate: String? = null,
    @SerialName("client_certificate")
    val clientCertificate: String? = null,
    @SerialName("client_private_key")
    val clientPrivateKey: String? = null
)

@Serializable
sealed class OtelExporterKind {
    @Serializable
    @SerialName("none")
    data object None : OtelExporterKind()

    @Serializable
    @SerialName("otlp-http")
    data class OtlpHttp(
        val endpoint: String,
        val headers: Map<String, String> = emptyMap(),
        val protocol: OtelHttpProtocol,
        val tls: OtelTlsConfig? = null
    ) : OtelExporterKind()

    @Serializable
    @SerialName("otlp-grpc")
    data class OtlpGrpc(
        val endpoint: String,
        val headers: Map<String, String> = emptyMap(),
        val tls: OtelTlsConfig? = null
    ) : OtelExporterKind()
}

/** OTEL settings loaded from config.toml. */
@Serializable
data class OtelConfigToml(
    @SerialName("log_user_prompt")
    val logUserPrompt: Boolean? = null,
    val environment: String? = null,
    val exporter: OtelExporterKind? = null
)

/** Effective OTEL settings after defaults are applied. */
data class OtelConfig(
    val logUserPrompt: Boolean = false,
    val environment: String = DEFAULT_OTEL_ENVIRONMENT,
    val exporter: OtelExporterKind = OtelExporterKind.None
)

// ============================================================================
// Notifications
// ============================================================================

@Serializable
sealed class Notifications {
    @Serializable
    @SerialName("enabled")
    data class Enabled(val value: Boolean) : Notifications()

    @Serializable
    @SerialName("custom")
    data class Custom(val commands: List<String>) : Notifications()

    companion object {
        fun default(): Notifications = Enabled(true)
    }
}

// ============================================================================
// TUI settings
// ============================================================================

@Serializable
data class Tui(
    val notifications: Notifications = Notifications.default(),
    val animations: Boolean = true
)

// ============================================================================
// Notice settings
// ============================================================================

@Serializable
data class Notice(
    @SerialName("hide_full_access_warning")
    val hideFullAccessWarning: Boolean? = null,
    @SerialName("hide_world_writable_warning")
    val hideWorldWritableWarning: Boolean? = null,
    @SerialName("hide_rate_limit_model_nudge")
    val hideRateLimitModelNudge: Boolean? = null,
    @SerialName("hide_gpt5_1_migration_prompt")
    val hideGpt51MigrationPrompt: Boolean? = null,
    @SerialName("hide_gpt-5.1-codex-max_migration_prompt")
    val hideGpt51CodexMaxMigrationPrompt: Boolean? = null
) {
    companion object {
        const val TABLE_KEY = "notice"
    }
}

// ============================================================================
// Sandbox settings
// ============================================================================

@Serializable
data class SandboxWorkspaceWrite(
    @SerialName("writable_roots")
    val writableRoots: List<String> = emptyList(),
    @SerialName("network_access")
    val networkAccess: Boolean = false,
    @SerialName("exclude_tmpdir_env_var")
    val excludeTmpdirEnvVar: Boolean = false,
    @SerialName("exclude_slash_tmp")
    val excludeSlashTmp: Boolean = false
)

// ============================================================================
// Shell environment policy
// ============================================================================

@Serializable
enum class ShellEnvironmentPolicyInherit {
    @SerialName("core")
    Core,

    @SerialName("all")
    All,

    @SerialName("none")
    None
}

@Serializable
data class ShellEnvironmentPolicyToml(
    val inherit: ShellEnvironmentPolicyInherit? = null,
    @SerialName("ignore_default_excludes")
    val ignoreDefaultExcludes: Boolean? = null,
    val exclude: List<String>? = null,
    val set: Map<String, String>? = null,
    @SerialName("include_only")
    val includeOnly: List<String>? = null,
    @SerialName("experimental_use_profile")
    val experimentalUseProfile: Boolean? = null
)

/**
 * Effective shell environment policy after defaults are applied.
 *
 * Deriving the env based on this policy:
 * 1. Create initial map based on `inherit`
 * 2. If `ignoreDefaultExcludes` is false, filter using default patterns (*KEY*, *TOKEN*)
 * 3. If `exclude` is not empty, filter using provided patterns
 * 4. Insert entries from `set`
 * 5. If non-empty, filter using `includeOnly` patterns
 */
data class ShellEnvironmentPolicy(
    val inherit: ShellEnvironmentPolicyInherit = ShellEnvironmentPolicyInherit.All,
    val ignoreDefaultExcludes: Boolean = false,
    val exclude: List<String> = emptyList(),
    val set: Map<String, String> = emptyMap(),
    val includeOnly: List<String> = emptyList(),
    val useProfile: Boolean = false
) {
    companion object {
        fun fromToml(toml: ShellEnvironmentPolicyToml): ShellEnvironmentPolicy {
            return ShellEnvironmentPolicy(
                inherit = toml.inherit ?: ShellEnvironmentPolicyInherit.All,
                ignoreDefaultExcludes = toml.ignoreDefaultExcludes ?: false,
                exclude = toml.exclude ?: emptyList(),
                set = toml.set ?: emptyMap(),
                includeOnly = toml.includeOnly ?: emptyList(),
                useProfile = toml.experimentalUseProfile ?: false
            )
        }
    }
}
