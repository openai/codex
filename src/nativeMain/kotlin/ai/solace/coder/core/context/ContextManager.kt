// port-lint: source core/src/context_manager/mod.rs
package ai.solace.coder.core.context

import ai.solace.coder.protocol.TokenUsage
import ai.solace.coder.protocol.TokenUsageInfo
import ai.solace.coder.protocol.FunctionCallOutputPayload
import ai.solace.coder.protocol.ResponseItem

/**
 * Transcript of conversation history with token tracking.
 *
 * Ported from Rust codex-rs/core/src/context_manager/history.rs
 */
class ContextManager {
    /** The oldest items are at the beginning of the list. */
    private val items = mutableListOf<ResponseItem>()
    private var tokenInfo: TokenUsageInfo? = TokenUsageInfo.newOrAppend(null, null, null)

    fun tokenInfo(): TokenUsageInfo? = tokenInfo?.copy()

    fun setTokenInfo(info: TokenUsageInfo?) {
        tokenInfo = info
    }

    fun setTokenUsageFull(contextWindow: Long) {
        val info = tokenInfo
        if (info != null) {
            tokenInfo = info.fillToContextWindow(contextWindow)
        } else {
            tokenInfo = TokenUsageInfo.fullContextWindow(contextWindow)
        }
    }

    /**
     * Record items into history. Items should be ordered from oldest to newest.
     */
    fun recordItems(newItems: List<ResponseItem>, policy: TruncationPolicy) {
        for (item in newItems) {
            val isGhostSnapshot = item is ResponseItem.GhostSnapshot
            if (!isApiMessage(item) && !isGhostSnapshot) {
                continue
            }

            val processed = processItem(item, policy)
            items.add(processed)
        }
    }

    /**
     * Get the history prepared for sending to the model.
     */
    fun getHistory(): List<ResponseItem> {
        normalizeHistory()
        return contents()
    }

    /**
     * Get the history for prompt, with GhostSnapshots removed.
     */
    fun getHistoryForPrompt(): List<ResponseItem> {
        val history = getHistory().toMutableList()
        removeGhostSnapshots(history)
        return history
    }

    /**
     * Remove the first (oldest) item from history.
     */
    fun removeFirstItem() {
        if (items.isNotEmpty()) {
            val removed = items.removeAt(0)
            removeCorrespondingFor(items, removed)
        }
    }

    /**
     * Replace the entire history.
     */
    fun replace(newItems: List<ResponseItem>) {
        items.clear()
        items.addAll(newItems)
    }

    /**
     * Update token info from usage data.
     */
    fun updateTokenInfo(usage: TokenUsage, modelContextWindow: Long?) {
        tokenInfo = TokenUsageInfo.newOrAppend(tokenInfo, usage, modelContextWindow)
    }

    /**
     * Get total token usage including reasoning tokens.
     */
    fun getTotalTokenUsage(): Long {
        val lastUsage = tokenInfo?.lastTokenUsage?.totalTokens ?: 0L
        val reasoningTokens = getNonLastReasoningItemsTokens()
        return lastUsage + reasoningTokens
    }

    /**
     * Returns a clone of the contents in the transcript.
     */
    fun contents(): List<ResponseItem> = items.toList()

    /**
     * Normalize history to ensure call/output pairs are matched.
     */
    private fun normalizeHistory() {
        ensureCallOutputsPresent(items)
        removeOrphanOutputs(items)
    }

    private fun removeGhostSnapshots(itemList: MutableList<ResponseItem>) {
        itemList.removeAll { it is ResponseItem.GhostSnapshot }
    }

    private fun processItem(item: ResponseItem, policy: TruncationPolicy): ResponseItem {
        val policyWithSerializationBudget = policy.mul(1.2)
        return when (item) {
            is ResponseItem.FunctionCallOutput -> {
                val truncated = truncateText(item.output.content, policyWithSerializationBudget)
                ResponseItem.FunctionCallOutput(
                    callId = item.callId,
                    output = FunctionCallOutputPayload(
                        content = truncated,
                        contentItems = item.output.contentItems,
                        success = item.output.success
                    )
                )
            }
            is ResponseItem.CustomToolCallOutput -> {
                val truncated = truncateText(item.output, policyWithSerializationBudget)
                ResponseItem.CustomToolCallOutput(
                    callId = item.callId,
                    output = truncated
                )
            }
            else -> item
        }
    }

    private fun getNonLastReasoningItemsTokens(): Long {
        // Find last user message index
        val lastUserIndex = items.indexOfLast { item ->
            item is ResponseItem.Message && item.role == "user"
        }
        if (lastUserIndex < 0) return 0L

        // Sum reasoning tokens before the last user message
        var totalReasoningBytes = 0
        for (i in 0 until lastUserIndex) {
            val item = items[i]
            if (item is ResponseItem.Reasoning && item.encryptedContent != null) {
                totalReasoningBytes += estimateReasoningLength(item.encryptedContent.length)
            }
        }

        return TruncationPolicy.approxTokensFromByteCount(totalReasoningBytes).toLong()
    }

    companion object {
        private fun estimateReasoningLength(encodedLen: Int): Int {
            return ((encodedLen * 3) / 4).coerceAtLeast(0) - 650
        }
    }
}

/**
 * Check if a response item is an API message (should be included in history).
 */
private fun isApiMessage(message: ResponseItem): Boolean {
    return when (message) {
        is ResponseItem.Message -> message.role != "system"
        is ResponseItem.FunctionCallOutput,
        is ResponseItem.FunctionCall,
        is ResponseItem.CustomToolCall,
        is ResponseItem.CustomToolCallOutput,
        is ResponseItem.LocalShellCall,
        is ResponseItem.Reasoning,
        is ResponseItem.WebSearchCall,
        is ResponseItem.CompactionSummary -> true
        is ResponseItem.GhostSnapshot -> false
        else -> false
    }
}

/**
 * Ensure every tool call has a corresponding output.
 */
private fun ensureCallOutputsPresent(items: MutableList<ResponseItem>) {
    val missingOutputsToInsert = mutableListOf<Pair<Int, ResponseItem>>()

    for ((idx, item) in items.withIndex()) {
        when (item) {
            is ResponseItem.FunctionCall -> {
                val callId = item.callId
                val hasOutput = items.any { it is ResponseItem.FunctionCallOutput && it.callId == callId }
                if (!hasOutput) {
                    missingOutputsToInsert.add(
                        idx to ResponseItem.FunctionCallOutput(
                            callId = callId,
                            output = FunctionCallOutputPayload(
                                content = "aborted",
                                contentItems = null,
                                success = null
                            )
                        )
                    )
                }
            }
            is ResponseItem.CustomToolCall -> {
                val callId = item.callId
                val hasOutput = items.any { it is ResponseItem.CustomToolCallOutput && it.callId == callId }
                if (!hasOutput) {
                    missingOutputsToInsert.add(
                        idx to ResponseItem.CustomToolCallOutput(
                            callId = callId,
                            output = "aborted"
                        )
                    )
                }
            }
            is ResponseItem.LocalShellCall -> {
                val callId = item.callId ?: continue
                val hasOutput = items.any { it is ResponseItem.FunctionCallOutput && it.callId == callId }
                if (!hasOutput) {
                    missingOutputsToInsert.add(
                        idx to ResponseItem.FunctionCallOutput(
                            callId = callId,
                            output = FunctionCallOutputPayload(
                                content = "aborted",
                                contentItems = null,
                                success = null
                            )
                        )
                    )
                }
            }
            else -> {}
        }
    }

    // Insert in reverse order to avoid index shifting
    for ((idx, outputItem) in missingOutputsToInsert.reversed()) {
        items.add(idx + 1, outputItem)
    }
}

/**
 * Remove outputs that don't have a corresponding call.
 */
private fun removeOrphanOutputs(items: MutableList<ResponseItem>) {
    val functionCallIds = items.filterIsInstance<ResponseItem.FunctionCall>()
        .map { it.callId }.toSet()
    val localShellCallIds = items.filterIsInstance<ResponseItem.LocalShellCall>()
        .mapNotNull { it.callId }.toSet()
    val customToolCallIds = items.filterIsInstance<ResponseItem.CustomToolCall>()
        .map { it.callId }.toSet()

    items.removeAll { item ->
        when (item) {
            is ResponseItem.FunctionCallOutput -> {
                !functionCallIds.contains(item.callId) && !localShellCallIds.contains(item.callId)
            }
            is ResponseItem.CustomToolCallOutput -> {
                !customToolCallIds.contains(item.callId)
            }
            else -> false
        }
    }
}

/**
 * Remove the corresponding call/output pair when one is removed.
 */
private fun removeCorrespondingFor(items: MutableList<ResponseItem>, item: ResponseItem) {
    when (item) {
        is ResponseItem.FunctionCall -> {
            val idx = items.indexOfFirst {
                it is ResponseItem.FunctionCallOutput && it.callId == item.callId
            }
            if (idx >= 0) items.removeAt(idx)
        }
        is ResponseItem.FunctionCallOutput -> {
            var idx = items.indexOfFirst {
                it is ResponseItem.FunctionCall && it.callId == item.callId
            }
            if (idx >= 0) {
                items.removeAt(idx)
            } else {
                idx = items.indexOfFirst {
                    it is ResponseItem.LocalShellCall && it.callId == item.callId
                }
                if (idx >= 0) items.removeAt(idx)
            }
        }
        is ResponseItem.CustomToolCall -> {
            val idx = items.indexOfFirst {
                it is ResponseItem.CustomToolCallOutput && it.callId == item.callId
            }
            if (idx >= 0) items.removeAt(idx)
        }
        is ResponseItem.CustomToolCallOutput -> {
            val idx = items.indexOfFirst {
                it is ResponseItem.CustomToolCall && it.callId == item.callId
            }
            if (idx >= 0) items.removeAt(idx)
        }
        is ResponseItem.LocalShellCall -> {
            val callId = item.callId ?: return
            val idx = items.indexOfFirst {
                it is ResponseItem.FunctionCallOutput && it.callId == callId
            }
            if (idx >= 0) items.removeAt(idx)
        }
        else -> {}
    }
}
