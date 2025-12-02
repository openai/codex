// port-lint: source codex-rs/protocol/src/message_history.rs
package ai.solace.coder.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Message history types.
 *
 * Ported from Rust codex-rs/protocol/src/message_history.rs
 */

@Serializable
data class HistoryEntry(
    @SerialName("conversation_id")
    val conversationId: String,
    val ts: Long,
    val text: String
)
