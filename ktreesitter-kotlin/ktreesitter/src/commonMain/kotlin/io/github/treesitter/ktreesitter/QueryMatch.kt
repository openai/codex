package io.github.treesitter.ktreesitter

import kotlin.jvm.JvmName

/**
 * A match that corresponds to a certain pattern in the query.
 *
 * @property patternIndex The index of the pattern.
 * @property captures The captures contained in the pattern.
 */
class QueryMatch internal constructor(
    @get:JvmName("getPatternIndex") val patternIndex: UInt,
    val captures: List<QueryCapture>
) {
    /** Get the nodes that are captured by the given [capture] name. */
    operator fun get(capture: String): List<Node> =
        captures.mapNotNull { if (it.name == capture) it.node else null }

    override fun toString() = "QueryMatch(patternIndex=$patternIndex, captures=$captures)"
}
