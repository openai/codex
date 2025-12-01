// port-lint: source protocol/src/items.rs
package ai.solace.coder.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Turn item types for conversation history.
 *
 * Ported from Rust codex-rs/protocol/src/items.rs
 */

@Serializable
sealed class TurnItem {
    @Serializable
    @SerialName("UserMessage")
    data class UserMessage(val item: UserMessageItem) : TurnItem()

    @Serializable
    @SerialName("AgentMessage")
    data class AgentMessage(val item: AgentMessageItem) : TurnItem()

    @Serializable
    @SerialName("Reasoning")
    data class Reasoning(val item: ReasoningItem) : TurnItem()

    @Serializable
    @SerialName("WebSearch")
    data class WebSearch(val item: WebSearchItem) : TurnItem()

    fun id(): String = when (this) {
        is UserMessage -> item.id
        is AgentMessage -> item.id
        is Reasoning -> item.id
        is WebSearch -> item.id
    }

    fun asLegacyEvents(showRawAgentReasoning: Boolean): List<EventMsg> = when (this) {
        is UserMessage -> listOf(item.asLegacyEvent())
        is AgentMessage -> item.asLegacyEvents()
        is WebSearch -> listOf(item.asLegacyEvent())
        is Reasoning -> item.asLegacyEvents(showRawAgentReasoning)
    }
}

@Serializable
data class UserMessageItem(
    val id: String,
    val content: List<UserInput>
) {
    companion object {
        fun new(content: List<UserInput>): UserMessageItem {
            return UserMessageItem(
                id = generateUuid(),
                content = content
            )
        }

        private fun generateUuid(): String {
            val chars = "0123456789abcdef"
            return buildString {
                repeat(8) { append(chars.random()) }
                append("-")
                repeat(4) { append(chars.random()) }
                append("-4") // Version 4
                repeat(3) { append(chars.random()) }
                append("-")
                append(chars.filter { it in "89ab" }.random())
                repeat(3) { append(chars.random()) }
                append("-")
                repeat(12) { append(chars.random()) }
            }
        }
    }

    fun asLegacyEvent(): EventMsg {
        return EventMsg.UserMessage(UserMessageEvent(
            message = message(),
            images = imageUrls().ifEmpty { null }
        ))
    }

    fun message(): String {
        return content.mapNotNull { c ->
            when (c) {
                is UserInput.Text -> c.text
                else -> null
            }
        }.joinToString("")
    }

    fun imageUrls(): List<String> {
        return content.mapNotNull { c ->
            when (c) {
                is UserInput.Image -> c.imageUrl
                else -> null
            }
        }
    }
}

@Serializable
sealed class AgentMessageContent {
    @Serializable
    @SerialName("Text")
    data class Text(val text: String) : AgentMessageContent()
}

@Serializable
data class AgentMessageItem(
    val id: String,
    val content: List<AgentMessageContent>
) {
    companion object {
        fun new(content: List<AgentMessageContent>): AgentMessageItem {
            return AgentMessageItem(
                id = generateUuid(),
                content = content
            )
        }

        private fun generateUuid(): String {
            val chars = "0123456789abcdef"
            return buildString {
                repeat(8) { append(chars.random()) }
                append("-")
                repeat(4) { append(chars.random()) }
                append("-4")
                repeat(3) { append(chars.random()) }
                append("-")
                append(chars.filter { it in "89ab" }.random())
                repeat(3) { append(chars.random()) }
                append("-")
                repeat(12) { append(chars.random()) }
            }
        }
    }

    fun asLegacyEvents(): List<EventMsg> {
        return content.map { c ->
            when (c) {
                is AgentMessageContent.Text -> EventMsg.AgentMessage(
                    AgentMessageEvent(message = c.text)
                )
            }
        }
    }
}

@Serializable
data class ReasoningItem(
    val id: String,
    @SerialName("summary_text")
    val summaryText: List<String>,
    @SerialName("raw_content")
    val rawContent: List<String> = emptyList()
) {
    fun asLegacyEvents(showRawAgentReasoning: Boolean): List<EventMsg> {
        val events = mutableListOf<EventMsg>()

        for (summary in summaryText) {
            events.add(EventMsg.AgentReasoning(AgentReasoningEvent(text = summary)))
        }

        if (showRawAgentReasoning) {
            for (entry in rawContent) {
                events.add(EventMsg.AgentReasoningRawContent(
                    AgentReasoningRawContentEvent(text = entry)
                ))
            }
        }

        return events
    }
}

@Serializable
data class WebSearchItem(
    val id: String,
    val query: String
) {
    fun asLegacyEvent(): EventMsg {
        return EventMsg.WebSearchEnd(WebSearchEndEvent(
            callId = id,
            query = query
        ))
    }
}
