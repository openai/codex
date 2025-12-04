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
