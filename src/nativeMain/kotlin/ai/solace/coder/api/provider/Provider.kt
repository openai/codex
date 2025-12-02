// port-lint: source codex-rs/codex-api/src/provider.rs
package ai.solace.coder.api.provider

import io.ktor.client.request.*
import io.ktor.http.*
import kotlin.time.Duration

/** Wire-level APIs supported by a Provider. */
enum class WireApi {
    Responses,
    Chat,
    Compact,
}

/** High-level retry configuration for a provider. */
data class RetryConfig(
    val maxAttempts: Long,
    val baseDelay: Duration,
    val retry429: Boolean,
    val retry5xx: Boolean,
    val retryTransport: Boolean,
)

/** HTTP endpoint configuration used to talk to a concrete API deployment. */
data class Provider(
    val name: String,
    val baseUrl: String,
    val queryParams: Map<String, String>?,
    val wire: WireApi,
    val defaultHeaders: Map<String, String>,
    val retry: RetryConfig,
    val streamIdleTimeout: Duration,
) {
    fun urlForPath(path: String): String {
        val base = baseUrl.trimEnd('/')
        val p = path.trimStart('/')
        var url = if (p.isEmpty()) base else "$base/$p"
        val params = queryParams
        if (params != null && params.isNotEmpty()) {
            val qs = params.entries.joinToString("&") { (k, v) -> "$k=$v" }
            url += "?$qs"
        }
        return url
    }

    fun buildRequest(method: HttpMethod, path: String, block: HttpRequestBuilder.() -> Unit = {}): HttpRequestBuilder {
        return HttpRequestBuilder().apply {
            this.method = method
            this.url(urlForPath(path))
            defaultHeaders.forEach { (key, value) ->
                headers.append(key, value)
            }
            block()
        }
    }

    fun isAzureResponsesEndpoint(): Boolean {
        if (wire != WireApi.Responses) return false
        if (name.equals("azure", ignoreCase = true)) return true
        val baseLower = baseUrl.lowercase()
        return baseLower.contains("openai.azure.") || matchesAzureResponsesBaseUrl(baseUrl)
    }

    companion object {
        private fun matchesAzureResponsesBaseUrl(baseUrl: String): Boolean {
            val AZURE_MARKERS = listOf(
                "cognitiveservices.azure.",
                "aoai.azure.",
                "azure-api.",
                "azurefd.",
                "windows.net/openai",
            )
            val base = baseUrl.lowercase()
            return AZURE_MARKERS.any { base.contains(it) }
        }
    }
}

