// port-lint: source codex-rs/codex-api/src/requests/headers.rs
package ai.solace.coder.api.requests

import io.ktor.client.request.*

/** Build conversation-related headers. */
internal fun buildConversationHeaders(conversationId: String?, builder: HttpRequestBuilder) {
    conversationId?.let { id ->
        builder.headers.append("conversation_id", id)
        builder.headers.append("session_id", id)
    }
}

/**
 * Extract subagent header value from SessionSource.
 *
 * Note: The Kotlin protocol currently has SessionSource as a simple enum.
 * In Rust, SessionSource::SubAgent(SubAgentSource) is a variant containing SubAgentSource.
 * For now, we check if SessionSource is SubAgent and return a default value.
 * TODO: Update Protocol.kt to use sealed class for SessionSource matching Rust structure.
 */
internal fun subagentHeader(source: ai.solace.coder.protocol.SessionSource?): String? {
    return when (source) {
        ai.solace.coder.protocol.SessionSource.SubAgent -> "review" // TODO: get actual SubAgentSource value
        else -> null
    }
}

/** Insert a header safely. */
internal fun insertHeader(builder: HttpRequestBuilder, name: String, value: String) {
    builder.headers.append(name, value)
}

