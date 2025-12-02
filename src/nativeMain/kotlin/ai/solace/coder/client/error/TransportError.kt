// port-lint: source codex-rs/codex-client/src/error.rs
package ai.solace.coder.client.error

import io.ktor.http.*

/**
 * Transport-level errors that can occur during HTTP communication.
 *
 * Mirrors Rust's TransportError from codex-client/src/error.rs
 */
sealed class TransportError : Exception() {
    /**
     * HTTP error with status code and optional response details.
     */
    data class Http(
        val status: HttpStatusCode,
        val headers: Headers? = null,
        val body: String? = null
    ) : TransportError() {
        override val message: String
            get() = "http ${status.value}: ${body ?: "no body"}"
    }

    /**
     * Maximum retry attempts exhausted.
     */
    object RetryLimit : TransportError() {
        override val message: String = "retry limit reached"
    }

    /**
     * Request timed out.
     */
    object Timeout : TransportError() {
        override val message: String = "timeout"
    }

    /**
     * Network-level error (connection failed, DNS resolution, etc.).
     */
    data class Network(override val message: String) : TransportError()

    /**
     * Error building the HTTP request.
     */
    data class Build(override val message: String) : TransportError()
}

/**
 * Streaming-specific errors.
 *
 * Mirrors Rust's StreamError from codex-client/src/error.rs
 */
sealed class StreamError : Exception() {
    /**
     * Stream failed during processing.
     */
    data class Stream(override val message: String) : StreamError()

    /**
     * Stream timed out.
     */
    object Timeout : StreamError() {
        override val message: String = "timeout"
    }
}

