package ai.solace.coder.core.config

import kotlinx.serialization.Serializable
import kotlin.time.Duration

const val DEFAULT_OTEL_ENVIRONMENT: String = "dev"

@Serializable
sealed class McpServerTransportConfig {
    @Serializable
    data class Stdio(
        val command: String,
        val args: List<String> = emptyList(),
        val env: Map<String, String>? = null,
        val envVars: List<String>? = null,
        val cwd: String? = null,
        val httpHeaders: Map<String, String>? = null,
        val envHttpHeaders: Map<String, String>? = null,
    ) : McpServerTransportConfig()

    @Serializable
    data class StreamableHttp(
        val url: String,
        val bearerToken: String? = null,
        val bearerTokenEnvVar: String? = null,
    ) : McpServerTransportConfig()
}

@Serializable
data class McpServerConfig(
    val transport: McpServerTransportConfig,
    val enabled: Boolean = true,
    val startupTimeout: Duration? = null,
    val toolTimeout: Duration? = null,
    val enabledTools: List<String>? = null,
    val disabledTools: List<String>? = null,
)

@Serializable
data class RawMcpServerConfig(
    // stdio
    val command: String? = null,
    val args: List<String>? = null,
    val env: Map<String, String>? = null,
    val envVars: List<String>? = null,
    val cwd: String? = null,
    val httpHeaders: Map<String, String>? = null,
    val envHttpHeaders: Map<String, String>? = null,
    // streamable_http
    val url: String? = null,
    val bearerToken: String? = null,
    val bearerTokenEnvVar: String? = null,
    // shared
    val startupTimeoutSec: Double? = null,
    val startupTimeoutMs: Long? = null,
    val toolTimeoutSec: Double? = null,
    val enabled: Boolean? = null,
    val enabledTools: List<String>? = null,
    val disabledTools: List<String>? = null,
) {
    fun normalize(): McpServerConfig {
        val startup = secondsToDuration(startupTimeoutSec) ?: millisToDuration(startupTimeoutMs)
        val tool = secondsToDuration(toolTimeoutSec)

        val isStdio = command != null || args != null || env != null || envVars != null || cwd != null || httpHeaders != null || envHttpHeaders != null
        val isHttp = url != null || bearerToken != null || bearerTokenEnvVar != null

        val transport = when {
            isStdio && isHttp -> error("Invalid MCP server config: mix of stdio and streamable_http fields")
            isHttp -> McpServerTransportConfig.StreamableHttp(
                url = url ?: error("Invalid MCP server config: missing 'url' for streamable_http"),
                bearerToken = bearerToken,
                bearerTokenEnvVar = bearerTokenEnvVar,
            )
            isStdio -> McpServerTransportConfig.Stdio(
                command = command ?: error("Invalid MCP server config: missing 'command' for stdio"),
                args = args ?: emptyList(),
                env = env,
                envVars = envVars,
                cwd = cwd,
                httpHeaders = httpHeaders,
                envHttpHeaders = envHttpHeaders,
            )
            else -> error("Invalid MCP server config: missing transport fields (need either 'url' or 'command')")
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
