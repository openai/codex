// port-lint: source codex-rs/codex-api/src/auth.rs
package ai.solace.coder.api

import io.ktor.client.request.*
import io.ktor.http.*

/**
 * Provides bearer and account identity information for API requests.
 *
 * Implementations should be cheap and non-blocking; any asynchronous
 * refresh or I/O should be handled by higher layers before requests
 * reach this interface.
 */
interface AuthProvider {
    fun bearerToken(): String?
    fun accountId(): String? = null
}

/**
 * Add authentication headers to a request.
 *
 * Mirrors Rust add_auth_headers by injecting Authorization and ChatGPT-Account-ID
 * when available. Extension function on HttpRequestBuilder for idiomatic usage.
 */
fun <A : AuthProvider> HttpRequestBuilder.addAuthHeaders(auth: A) {
    auth.bearerToken()?.let { token ->
        headers.append(HttpHeaders.Authorization, "Bearer $token")
    }
    auth.accountId()?.let { accountId ->
        headers.append("ChatGPT-Account-ID", accountId)
    }
}
