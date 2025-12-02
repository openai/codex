// port-lint: source codex-rs/protocol/src/conversation_id.rs
package ai.solace.coder.protocol

import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid
import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder

/**
 * Conversation ID wrapper.
 *
 * Ported from Rust codex-rs/protocol/src/conversation_id.rs
 */
@Serializable(with = ConversationIdSerializer::class)
data class ConversationId(
    private val uuid: String
) {
    companion object {
        fun new(): ConversationId {
            return ConversationId(generateUuidV7())
        }

        fun default(): ConversationId {
            return ConversationId("00000000-0000-0000-0000-000000000000")
        }

        fun fromString(s: String): kotlin.Result<ConversationId> {
            return runCatching {
                // Basic UUID validation
                if (s.length == 36 && s.count { it == '-' } == 4) {
                    ConversationId(s)
                } else {
                    throw IllegalArgumentException("Invalid UUID format: $s")
                }
            }
        }

        @OptIn(ExperimentalUuidApi::class)
        private fun generateUuidV7(): String {
            // Use Kotlin's built-in UUID (random v4 for now, close enough for unique IDs)
            return Uuid.random().toString()
        }
    }

    override fun toString(): String = uuid
}

object ConversationIdSerializer : KSerializer<ConversationId> {
    override val descriptor: SerialDescriptor =
        PrimitiveSerialDescriptor("ConversationId", PrimitiveKind.STRING)

    override fun serialize(encoder: Encoder, value: ConversationId) {
        encoder.encodeString(value.toString())
    }

    override fun deserialize(decoder: Decoder): ConversationId {
        return ConversationId.fromString(decoder.decodeString()).getOrThrow()
    }
}
