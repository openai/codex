// port-lint: source codex-rs/core/src/model_provider_info.rs
package ai.solace.coder.core.model

import ai.solace.coder.api.provider.Provider
import ai.solace.coder.api.provider.RetryConfig
import ai.solace.coder.api.provider.WireApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds

private const val DEFAULT_STREAM_IDLE_TIMEOUT_MS: Long = 300_000
private const val DEFAULT_STREAM_MAX_RETRIES: Long = 5
private const val DEFAULT_REQUEST_MAX_RETRIES: Long = 4
/** Hard cap for user-configured `stream_max_retries`. */
private const val MAX_STREAM_MAX_RETRIES: Long = 100
/** Hard cap for user-configured `request_max_retries`. */
private const val MAX_REQUEST_MAX_RETRIES: Long = 100

const val DEFAULT_LMSTUDIO_PORT: Int = 1234
const val DEFAULT_OLLAMA_PORT: Int = 11434

const val LMSTUDIO_OSS_PROVIDER_ID: String = "lmstudio"
const val OLLAMA_OSS_PROVIDER_ID: String = "ollama"

/**
 * Authentication mode for providers.
 */
enum class AuthMode {
    ApiKey,
    ChatGPT
}

/**
 * Serializable representation of a provider definition.
 *
 * Ported from Rust codex-rs/core/src/model_provider_info.rs
 */
@Serializable
data class ModelProviderInfo(
    /** Friendly display name. */
    val name: String,

    /** Base URL for the provider's OpenAI-compatible API. */
    @SerialName("base_url")
    val baseUrl: String? = null,

    /** Environment variable that stores the user's API key for this provider. */
    @SerialName("env_key")
    val envKey: String? = null,

    /** Optional instructions to help the user get a valid value for the variable and set it. */
    @SerialName("env_key_instructions")
    val envKeyInstructions: String? = null,

    /** Value to use with `Authorization: Bearer <token>` header. */
    @SerialName("experimental_bearer_token")
    val experimentalBearerToken: String? = null,

    /** Which wire protocol this provider expects. */
    @SerialName("wire_api")
    val wireApi: WireApi = WireApi.Chat,

    /** Optional query parameters to append to the base URL. */
    @SerialName("query_params")
    val queryParams: Map<String, String>? = null,

    /** Additional HTTP headers to include in requests to this provider. */
    @SerialName("http_headers")
    val httpHeaders: Map<String, String>? = null,

    /** HTTP headers from environment variables. */
    @SerialName("env_http_headers")
    val envHttpHeaders: Map<String, String>? = null,

    /** Maximum number of times to retry a failed HTTP request to this provider. */
    @SerialName("request_max_retries")
    val requestMaxRetries: Long? = null,

    /** Number of times to retry reconnecting a dropped streaming response before failing. */
    @SerialName("stream_max_retries")
    val streamMaxRetries: Long? = null,

    /** Idle timeout (in milliseconds) to wait for activity on a streaming response. */
    @SerialName("stream_idle_timeout_ms")
    val streamIdleTimeoutMs: Long? = null,

    /** Does this provider require an OpenAI API Key or ChatGPT login token? */
    @SerialName("requires_openai_auth")
    val requiresOpenAiAuth: Boolean = false
) {
    /**
     * Convert to API provider for use with HTTP client.
     */
    fun toApiProvider(authMode: AuthMode?): Provider {
        val defaultBaseUrl = if (authMode == AuthMode.ChatGPT) {
            "https://chatgpt.com/backend-api/codex"
        } else {
            "https://api.openai.com/v1"
        }
        val actualBaseUrl = baseUrl ?: defaultBaseUrl

        val headers = buildHeaders()
        val retry = RetryConfig(
            maxAttempts = requestMaxRetries(),
            baseDelay = 200.milliseconds,
            retry429 = false,
            retry5xx = true,
            retryTransport = true
        )

        return Provider(
            name = name,
            baseUrl = actualBaseUrl,
            queryParams = queryParams,
            wire = wireApi,
            defaultHeaders = headers,
            retry = retry,
            streamIdleTimeout = streamIdleTimeout()
        )
    }

    /**
     * Build HTTP headers from config.
     */
    private fun buildHeaders(): Map<String, String> {
        val headers = mutableMapOf<String, String>()

        httpHeaders?.forEach { (k, v) ->
            headers[k] = v
        }

        // Note: In Kotlin Native, environment variable access is platform-specific
        // This is a simplified implementation
        envHttpHeaders?.forEach { (header, envVar) ->
            // In a real implementation, this would read from platform env vars
            // For now, we skip env-based headers
        }

        return headers
    }

    /**
     * If `envKey` is Some, returns the API key for this provider if present.
     * Note: Environment variable access in Kotlin Native is platform-specific.
     */
    fun apiKey(): String? {
        // In a real implementation, this would read from platform env vars
        return null
    }

    /**
     * Effective maximum number of request retries for this provider.
     */
    fun requestMaxRetries(): Long {
        return (requestMaxRetries ?: DEFAULT_REQUEST_MAX_RETRIES).coerceAtMost(MAX_REQUEST_MAX_RETRIES)
    }

    /**
     * Effective maximum number of stream reconnection attempts for this provider.
     */
    fun streamMaxRetries(): Long {
        return (streamMaxRetries ?: DEFAULT_STREAM_MAX_RETRIES).coerceAtMost(MAX_STREAM_MAX_RETRIES)
    }

    /**
     * Effective idle timeout for streaming responses.
     */
    fun streamIdleTimeout(): Duration {
        return (streamIdleTimeoutMs ?: DEFAULT_STREAM_IDLE_TIMEOUT_MS).milliseconds
    }
}

/**
 * Built-in default provider list.
 */
fun builtInModelProviders(): Map<String, ModelProviderInfo> {
    return mapOf(
        "openai" to ModelProviderInfo(
            name = "OpenAI",
            baseUrl = null, // Uses default
            envKey = null,
            wireApi = WireApi.Responses,
            httpHeaders = mapOf("version" to "1.0.0"),
            envHttpHeaders = mapOf(
                "OpenAI-Organization" to "OPENAI_ORGANIZATION",
                "OpenAI-Project" to "OPENAI_PROJECT"
            ),
            requiresOpenAiAuth = true
        ),
        OLLAMA_OSS_PROVIDER_ID to createOssProvider(DEFAULT_OLLAMA_PORT, WireApi.Chat),
        LMSTUDIO_OSS_PROVIDER_ID to createOssProvider(DEFAULT_LMSTUDIO_PORT, WireApi.Responses)
    )
}

/**
 * Create an OSS provider with the given port and wire API.
 */
fun createOssProvider(defaultProviderPort: Int, wireApi: WireApi): ModelProviderInfo {
    val baseUrl = "http://localhost:$defaultProviderPort/v1"
    return createOssProviderWithBaseUrl(baseUrl, wireApi)
}

/**
 * Create an OSS provider with the given base URL and wire API.
 */
fun createOssProviderWithBaseUrl(baseUrl: String, wireApi: WireApi): ModelProviderInfo {
    return ModelProviderInfo(
        name = "gpt-oss",
        baseUrl = baseUrl,
        envKey = null,
        wireApi = wireApi,
        requiresOpenAiAuth = false
    )
}
