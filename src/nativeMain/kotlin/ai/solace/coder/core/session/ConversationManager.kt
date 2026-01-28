// port-lint: source codex-rs/core/src/conversation_manager.rs
package ai.solace.coder.core.session

import ai.solace.coder.client.auth.AuthManager
import ai.solace.coder.core.config.Config
import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.protocol.ConversationId
import ai.solace.coder.protocol.EventMsg
import ai.solace.coder.protocol.SessionConfiguredEvent
import ai.solace.coder.protocol.SessionSource
import ai.solace.coder.protocol.ResponseItem
import ai.solace.coder.protocol.ContentItem
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * Represents a newly created Codex conversation, including the first event
 * (which is SessionConfigured).
 */
data class NewConversation(
    val conversationId: ConversationId,
    val conversation: CodexConversation,
    val sessionConfigured: SessionConfiguredEvent
)

/**
 * Wrapper around a Codex instance that represents an active conversation.
 */
class CodexConversation(
    val codex: Codex,
    val rolloutPath: String?
)

/**
 * The initial submit ID used for the session configuration event.
 */
const val INITIAL_SUBMIT_ID: String = "initial"

/**
 * Initial history for conversation creation.
 */
sealed class InitialHistory {
    /** Brand new conversation. */
    data object New : InitialHistory()
    /** Forked from existing conversation with items. */
    data class Forked(val items: List<RolloutItem>) : InitialHistory()

    fun getRolloutItems(): List<RolloutItem> = when (this) {
        is New -> emptyList()
        is Forked -> items
    }
}

/**
 * An item in a conversation rollout.
 */
sealed class RolloutItem {
    data class ResponseItemWrapper(val item: ResponseItem) : RolloutItem()
}

/**
 * ConversationManager is responsible for creating conversations and
 * maintaining them in memory.
 *
 * Ported from Rust codex-rs/core/src/conversation_manager.rs
 */
class ConversationManager(
    private val authManager: AuthManager,
    private val sessionSource: SessionSource
) {
    private val conversations = mutableMapOf<ConversationId, CodexConversation>()
    private val mutex = Mutex()

    fun sessionSource(): SessionSource = sessionSource

    /**
     * Create a new conversation with the given config.
     */
    suspend fun newConversation(config: Config): CodexResult<NewConversation> {
        return spawnConversation(config, authManager)
    }

    /**
     * Get an existing conversation by its ID.
     */
    suspend fun getConversation(conversationId: ConversationId): CodexResult<CodexConversation> {
        return mutex.withLock {
            val conversation = conversations[conversationId]
            if (conversation != null) {
                CodexResult.success(conversation)
            } else {
                CodexResult.failure(CodexError.ConversationNotFound(conversationId.toString()))
            }
        }
    }

    /**
     * Remove a conversation from the manager.
     */
    suspend fun removeConversation(conversationId: ConversationId): CodexConversation? {
        return mutex.withLock {
            conversations.remove(conversationId)
        }
    }

    /**
     * Resume a conversation with the given initial history.
     */
    suspend fun resumeConversationWithHistory(
        config: Config,
        initialHistory: InitialHistory,
        authManager: AuthManager
    ): CodexResult<NewConversation> {
        // TODO: Spawn with initial history once Codex.spawn supports it
        return spawnConversation(config, authManager)
    }

    /**
     * Fork an existing conversation by taking messages up to the given position.
     */
    suspend fun forkConversation(
        nthUserMessage: Int,
        config: Config,
        rolloutPath: String
    ): CodexResult<NewConversation> {
        // TODO: Load rollout history and truncate
        return spawnConversation(config, authManager)
    }

    private suspend fun spawnConversation(
        config: Config,
        authManager: AuthManager
    ): CodexResult<NewConversation> {
        // TODO: Call Codex.spawn() once it's fully implemented
        // The flow is:
        // 1. Codex.spawn(config, authManager, InitialHistory.New, sessionSource)
        // 2. Wait for first SessionConfigured event
        // 3. Create CodexConversation and register it
        return CodexResult.failure(
            CodexError.NotImplemented("ConversationManager.spawnConversation requires full Codex.spawn implementation")
        )
    }

    /**
     * Finalize a spawned conversation by validating the first event
     * and registering the conversation.
     */
    internal suspend fun finalizeSpawn(
        codex: Codex,
        conversationId: ConversationId,
        rolloutPath: String?
    ): CodexResult<NewConversation> {
        // The first event must be SessionConfigured. Validate and forward it
        // to the caller so they can display it in the conversation history.
        val eventResult = codex.nextEvent()
        val event = eventResult.getOrNull()
            ?: return CodexResult.failure(CodexError.SessionConfiguredNotFirstEvent)

        val sessionConfigured = when (val msg = event.msg) {
            is EventMsg.SessionConfigured -> msg.payload
            else -> return CodexResult.failure(CodexError.SessionConfiguredNotFirstEvent)
        }

        val conversation = CodexConversation(codex, rolloutPath)

        mutex.withLock {
            conversations[conversationId] = conversation
        }

        return CodexResult.success(
            NewConversation(
                conversationId = conversationId,
                conversation = conversation,
                sessionConfigured = sessionConfigured
            )
        )
    }
}

/**
 * Return a prefix of items obtained by cutting strictly before the nth user
 * message (0-based) and all items that follow it.
 */
fun truncateBeforeNthUserMessage(history: InitialHistory, n: Int): InitialHistory {
    val items = history.getRolloutItems()

    // Find indices of user message inputs in rollout order
    val userPositions = mutableListOf<Int>()
    for ((idx, item) in items.withIndex()) {
        if (item is RolloutItem.ResponseItemWrapper) {
            val responseItem = item.item
            if (responseItem is ResponseItem.Message && responseItem.role == "user") {
                userPositions.add(idx)
            }
        }
    }

    // If fewer than or equal to n user messages exist, treat as empty
    if (userPositions.size <= n) {
        return InitialHistory.New
    }

    // Cut strictly before the nth user message
    val cutIdx = userPositions[n]
    val rolled = items.take(cutIdx)

    return if (rolled.isEmpty()) {
        InitialHistory.New
    } else {
        InitialHistory.Forked(rolled)
    }
}
