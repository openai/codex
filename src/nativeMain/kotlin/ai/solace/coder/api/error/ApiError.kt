// port-lint: source codex-rs/codex-api/src/error.rs
package ai.solace.coder.api.error

import ai.solace.coder.client.error.TransportError
import io.ktor.http.*
import kotlin.time.Duration

/**
 * API error types.
 *
 * Mirrors Rust's ApiError from codex-api/src/error.rs
 */
sealed class ApiError : Exception() {
    /**
     * Transport-level error (network, timeout, retry limit, etc.).
     * Maps from TransportError.
     */
    data class Transport(val error: TransportError) : ApiError() {
        override val message: String
            get() = error.message ?: "transport error"
    }

    /**
     * API returned an error status code.
     */
    data class Api(val status: HttpStatusCode, override val message: String) : ApiError()

    /**
     * Streaming error.
     */
    data class Stream(override val message: String) : ApiError()

    /**
     * Context window size exceeded for the model.
     */
    data class ContextWindowExceeded(val details: String? = null) : ApiError() {
        override val message: String
            get() = details?.let { "context window exceeded: $it" }
                ?: "context window exceeded"
    }

    /**
     * API quota exceeded.
     */
    data class QuotaExceeded(val details: String? = null) : ApiError() {
        override val message: String
            get() = details?.let { "quota exceeded: $it" }
                ?: "quota exceeded"
    }

    /**
     * Usage information not included in response.
     */
    data class UsageNotIncluded(val details: String? = null) : ApiError() {
        override val message: String
            get() = details?.let { "usage not included: $it" }
                ?: "usage not included"
    }

    /**
     * Retryable error with optional delay hint.
     */
    data class Retryable(
        override val message: String,
        val delay: Duration? = null
    ) : ApiError()

    /**
     * Rate limit error.
     */
    data class RateLimit(override val message: String) : ApiError()
}


