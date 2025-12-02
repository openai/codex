// port-lint: source codex-rs/codex-api/src/sse/mod.rs, codex-rs/codex-api/src/sse/chat.rs, codex-rs/codex-api/src/sse/responses.rs
package ai.solace.coder.api.sse

import ai.solace.coder.api.common.ResponseEvent
import ai.solace.coder.api.common.ResponseStream
import ai.solace.coder.api.telemetry.SseTelemetry
import io.ktor.client.*
import kotlin.time.Duration

/**
 * Spawn a chat stream parser from an SSE stream.
 * TODO: Implement full SSE parsing with eventsource-stream equivalent.
 */
suspend fun spawnChatStream(
    httpClient: HttpClient,
    url: String,
    idleTimeout: Duration,
    telemetry: SseTelemetry?,
): ResponseStream {
    TODO("spawnChatStream: SSE parsing not yet implemented")
}

/**
 * Spawn a responses stream parser from an SSE stream.
 * TODO: Implement full SSE parsing with eventsource-stream equivalent.
 */
suspend fun spawnResponsesStream(
    httpClient: HttpClient,
    url: String,
    idleTimeout: Duration,
    telemetry: SseTelemetry?,
): ResponseStream {
    TODO("spawnResponsesStream: SSE parsing not yet implemented")
}

/**
 * Load an SSE stream from a test fixture file.
 * TODO: Implement for testing purposes.
 */
fun streamFromFixture(path: String): ResponseStream {
    TODO("streamFromFixture: test fixture loading not yet implemented")
}

/**
 * Parse SSE event data into ResponseEvent.
 * TODO: Implement JSON parsing for various event types.
 */
internal fun parseResponseEvent(eventType: String, data: String): Result<ResponseEvent> {
    TODO("parseResponseEvent: event parsing not yet implemented")
}

