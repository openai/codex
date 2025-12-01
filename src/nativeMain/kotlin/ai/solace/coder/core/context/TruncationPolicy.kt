// port-lint: source core/src/context_manager/normalize.rs
package ai.solace.coder.core.context

/**
 * Policy for truncating large chunks of output while preserving a prefix
 * and suffix on UTF-8 boundaries.
 *
 * Ported from Rust codex-rs/core/src/truncate.rs
 */
sealed class TruncationPolicy {
    data class Bytes(val bytes: Int) : TruncationPolicy()
    data class Tokens(val tokens: Int) : TruncationPolicy()

    /**
     * Scale the underlying budget by multiplier, rounding up to avoid under-budgeting.
     */
    fun mul(multiplier: Double): TruncationPolicy {
        return when (this) {
            is Bytes -> Bytes((bytes.toDouble() * multiplier).toInt() + 1)
            is Tokens -> Tokens((tokens.toDouble() * multiplier).toInt() + 1)
        }
    }

    /**
     * Returns a token budget derived from this policy.
     */
    fun tokenBudget(): Int {
        return when (this) {
            is Bytes -> approxTokensFromByteCount(bytes)
            is Tokens -> tokens
        }
    }

    /**
     * Returns a byte budget derived from this policy.
     */
    fun byteBudget(): Int {
        return when (this) {
            is Bytes -> bytes
            is Tokens -> approxBytesForTokens(tokens)
        }
    }

    companion object {
        const val APPROX_BYTES_PER_TOKEN = 4

        fun approxTokenCount(text: String): Int {
            val len = text.length
            return (len + APPROX_BYTES_PER_TOKEN - 1) / APPROX_BYTES_PER_TOKEN
        }

        fun approxBytesForTokens(tokens: Int): Int {
            return tokens * APPROX_BYTES_PER_TOKEN
        }

        fun approxTokensFromByteCount(bytes: Int): Int {
            return (bytes + APPROX_BYTES_PER_TOKEN - 1) / APPROX_BYTES_PER_TOKEN
        }
    }
}

/**
 * Truncate text with formatted output showing total lines.
 */
fun formattedTruncateText(content: String, policy: TruncationPolicy): String {
    if (content.length <= policy.byteBudget()) {
        return content
    }
    val totalLines = content.lines().size
    val result = truncateText(content, policy)
    return "Total output lines: $totalLines\n\n$result"
}

/**
 * Truncate text according to policy.
 */
fun truncateText(content: String, policy: TruncationPolicy): String {
    return when (policy) {
        is TruncationPolicy.Bytes -> truncateWithByteEstimate(content, policy)
        is TruncationPolicy.Tokens -> truncateWithByteEstimate(content, policy)
    }
}

/**
 * Truncate a string using a byte budget, preserving beginning and end.
 */
private fun truncateWithByteEstimate(s: String, policy: TruncationPolicy): String {
    if (s.isEmpty()) {
        return ""
    }

    val totalChars = s.length
    val maxBytes = policy.byteBudget()

    if (maxBytes == 0) {
        val marker = formatTruncationMarker(policy, totalChars.toLong())
        return marker
    }

    if (s.length <= maxBytes) {
        return s
    }

    val (leftBudget, rightBudget) = splitBudget(maxBytes)
    val (removedChars, left, right) = splitString(s, leftBudget, rightBudget)

    val marker = formatTruncationMarker(policy, removedChars.toLong())
    return assembleTruncatedOutput(left, right, marker)
}

/**
 * Split string preserving UTF-8 boundaries.
 */
private fun splitString(s: String, beginningBytes: Int, endBytes: Int): Triple<Int, String, String> {
    if (s.isEmpty()) {
        return Triple(0, "", "")
    }

    val len = s.length
    val tailStartTarget = (len - endBytes).coerceAtLeast(0)
    var prefixEnd = 0
    var suffixStart = len
    var removedChars = 0
    var suffixStarted = false

    for ((idx, char) in s.withIndex()) {
        val charEnd = idx + 1
        if (charEnd <= beginningBytes) {
            prefixEnd = charEnd
            continue
        }

        if (idx >= tailStartTarget) {
            if (!suffixStarted) {
                suffixStart = idx
                suffixStarted = true
            }
            continue
        }

        removedChars++
    }

    if (suffixStart < prefixEnd) {
        suffixStart = prefixEnd
    }

    val before = s.substring(0, prefixEnd)
    val after = s.substring(suffixStart)

    return Triple(removedChars, before, after)
}

private fun formatTruncationMarker(policy: TruncationPolicy, removedCount: Long): String {
    return when (policy) {
        is TruncationPolicy.Tokens -> "…$removedCount tokens truncated…"
        is TruncationPolicy.Bytes -> "…$removedCount chars truncated…"
    }
}

private fun splitBudget(budget: Int): Pair<Int, Int> {
    val left = budget / 2
    return Pair(left, budget - left)
}

private fun assembleTruncatedOutput(prefix: String, suffix: String, marker: String): String {
    return buildString(prefix.length + marker.length + suffix.length + 1) {
        append(prefix)
        append(marker)
        append(suffix)
    }
}
