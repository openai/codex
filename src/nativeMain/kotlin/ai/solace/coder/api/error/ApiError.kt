// port-lint: source codex-rs/codex-api/src/error.rs
package ai.solace.coder.api.error

import kotlin.time.Duration

/**
 * API error types.
 *
 * TODO: Map TransportError and StatusCode once transport/types are ported.
 */
sealed class ApiError {
    data class Transport(val message: String) : ApiError() // TODO: replace with TransportError type
    data class Api(val status: Int, val message: String) : ApiError()
    data class Stream(val message: String) : ApiError()
    data class ContextWindowExceeded(val details: String? = null) : ApiError()
    data class QuotaExceeded(val details: String? = null) : ApiError()
    data class UsageNotIncluded(val details: String? = null) : ApiError()
    data class Retryable(val message: String, val delay: Duration? = null) : ApiError()
    data class RateLimit(val message: String) : ApiError()
}

