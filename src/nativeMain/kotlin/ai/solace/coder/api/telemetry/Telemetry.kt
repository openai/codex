// port-lint: source codex-rs/codex-api/src/telemetry.rs
package ai.solace.coder.api.telemetry

import io.ktor.http.*
import kotlin.time.Duration

/** Generic telemetry for SSE event streams. */
interface SseTelemetry {
    /**
     * Called on each SSE poll with the result and duration.
     * TODO: Define proper event/error types once eventsource streaming is implemented.
     */
    fun onSsePoll(result: Result<Any?>, duration: Duration)
}

/** Telemetry for individual HTTP requests. */
interface RequestTelemetry {
    /**
     * Called after each request attempt.
     * @param attempt The attempt number (1-indexed)
     * @param status HTTP status code if available
     * @param error Transport error if the request failed
     * @param duration Time taken for the request
     */
    fun onRequest(attempt: Int, status: HttpStatusCode?, error: Throwable?, duration: Duration)
}

/**
 * Execute a request with retry and telemetry.
 * TODO: Implement full retry policy and telemetry integration with Ktor client.
 */
suspend fun <T> runWithRequestTelemetry(
    telemetry: RequestTelemetry?,
    attempt: Int,
    block: suspend () -> T,
): Result<T> {
    val timeSource = kotlin.time.TimeSource.Monotonic
    val startMark = timeSource.markNow()
    return try {
        val result = block()
        val duration = startMark.elapsedNow()
        telemetry?.onRequest(attempt, null, null, duration)
        Result.success(result)
    } catch (e: Exception) {
        val duration = startMark.elapsedNow()
        telemetry?.onRequest(attempt, null, e, duration)
        Result.failure(e)
    }
}

