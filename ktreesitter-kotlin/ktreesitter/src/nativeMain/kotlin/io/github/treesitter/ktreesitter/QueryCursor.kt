package io.github.treesitter.ktreesitter

import io.github.treesitter.ktreesitter.internal.*
import kotlin.experimental.ExperimentalNativeApi
import kotlin.native.ref.createCleaner
import kotlinx.cinterop.*

/**
 * A class that is used for executing a query.
 *
 * @since 0.25.0
 */
@OptIn(ExperimentalForeignApi::class)
actual class QueryCursor internal constructor(
    private val query: Query,
    private val node: Node,
    progressCallback: QueryProgressCallback? = null
) {
    internal val self = ts_query_cursor_new()!!

    init {
        if (progressCallback == null) {
            ts_query_cursor_exec(self, query.self, node.self)
        } else {
            val progressRef = StableRef.create(progressCallback)
            val options = cValue<TSQueryCursorOptions> {
                payload = progressRef.asCPointer()
                progress_callback = staticCFunction { state ->
                    val callback = state!!.pointed.payload!!
                        .asStableRef<QueryProgressCallback>().get()
                    callback(state.pointed.current_byte_offset)
                }
            }
            ts_query_cursor_exec_with_options(self, query.self, node.self, options)
            progressRef.dispose()
        }
    }

    @Suppress("unused")
    @OptIn(ExperimentalNativeApi::class)
    private val cleaner = createCleaner(self, ::ts_query_cursor_delete)

    /**
     * The maximum duration in microseconds that query
     * execution should be allowed to take before halting.
     *
     * Default: `0`
     *
     * @since 0.23.0
     */
    @Deprecated("Use the progressCallback in Query.invoke()")
    actual var timeoutMicros: ULong
        get() = ts_query_cursor_timeout_micros(self)
        set(value) {
            ts_query_cursor_set_timeout_micros(self, value)
        }

    /**
     * The maximum number of in-progress matches.
     *
     * Default: `UInt.MAX_VALUE`
     *
     * @throws [IllegalArgumentException] If the match limit is set to `0`.
     */
    actual var matchLimit: UInt
        get() = ts_query_cursor_match_limit(self)
        set(value) {
            require(value > 0U) { "The match limit cannot be 0" }
            ts_query_cursor_set_match_limit(self, value)
        }

    /**
     * The maximum start depth for the query.
     *
     * This prevents cursors from exploring children nodes at a certain depth.
     * Note that if a pattern includes many children, then they will still be checked.
     *
     * Default: `UInt.MAX_VALUE`
     */
    actual var maxStartDepth: UInt = UInt.MAX_VALUE
        set(value) {
            ts_query_cursor_set_max_start_depth(self, value)
            field = value
        }

    /**
     * The range of bytes in which the query will be executed.
     *
     * The query cursor will return matches that intersect with
     * the given range. This means that a match may be returned
     * even if some of its captures fall outside the specified range,
     * as long as at least part of the match overlaps with the range.
     *
     * Default: `UInt.MIN_VALUE..UInt.MAX_VALUE`
     *
     * @throws [IllegalArgumentException] If set to an invalid range.
     */
    actual var byteRange: UIntRange = UInt.MIN_VALUE..UInt.MAX_VALUE
        set(value) {
            require(ts_query_cursor_set_byte_range(self, value.first, value.last)) {
                "Invalid byte range: [${value.first}, ${value.last}]"
            }
            field = value
        }

    /**
     * The range of points in which the query will be executed.
     *
     * The query cursor will return matches that intersect with
     * the given range. This means that a match may be returned
     * even if some of its captures fall outside the specified range,
     * as long as at least part of the match overlaps with the range.
     *
     * Default: `Point.MIN..Point.MAX`
     *
     * @throws [IllegalArgumentException] If set to an invalid range.
     */
    actual var pointRange: ClosedRange<Point> = Point.MIN..Point.MAX
        set(value) {
            val start = cValue<TSPoint> { from(value.start) }
            val end = cValue<TSPoint> { from(value.endInclusive) }
            require(ts_query_cursor_set_point_range(self, start, end)) {
                "Invalid point range: [${value.start}, ${value.endInclusive}]"
            }
            field = value
        }

    /**
     * Check if the query exceeded its maximum number of
     * in-progress matches during its last execution.
     *
     * @see matchLimit
     */
    actual val didExceedMatchLimit: Boolean
        get() = ts_query_cursor_did_exceed_match_limit(self)

    /**
     * Iterate over all the matches in the order that they were found.
     *
     * #### Example
     *
     * ```kotlin
     * query(tree.rootNode).matches {
     *      if (name != "ieq?") return@matches true
     *      val node = it[(args[0] as QueryPredicateArg.Capture).value].first()
     *      val value = (args[1] as QueryPredicateArg.Literal).value
     *      value.equals(node.text()?.toString(), ignoreCase = true)
     *  }
     * ```
     *
     * @param predicate A function that handles custom predicates.
     */
    actual fun matches(predicate: QueryPredicate.(QueryMatch) -> Boolean) = sequence<QueryMatch> {
        memScoped {
            val match = alloc<TSQueryMatch>()
            while (ts_query_cursor_next_match(self, match.ptr)) {
                match.convert(predicate)?.let { yield(it) }
            }
        }
    }

    /**
     * Iterate over all the individual captures in the order that they appear.
     *
     * This is useful if you don't care about _which_ pattern matched.
     *
     * @param predicate A function that handles custom predicates.
     */
    actual fun captures(predicate: QueryPredicate.(QueryMatch) -> Boolean) =
        sequence<Pair<UInt, QueryMatch>> {
            memScoped {
                val match = alloc<TSQueryMatch>()
                val index = alloc<UIntVar>()
                while (ts_query_cursor_next_capture(self, match.ptr, index.ptr)) {
                    match.convert(predicate)?.let { yield(index.value to it) }
                }
            }
        }

    override fun toString() = "QueryCursor(query=$query, node=$node)"

    private fun TSQueryMatch.convert(
        predicate: QueryPredicate.(QueryMatch) -> Boolean
    ): QueryMatch? {
        val index = pattern_index.convert<UInt>()
        val captures = (UShort.MIN_VALUE..<capture_count).map {
            val c = captures!![it.convert<Long>()]
            QueryCapture(
                Node(c.node.readValue(), node.tree),
                query.captureNames[c.index.toInt()]
            )
        }
        return QueryMatch(index, captures).takeIf { match ->
            node.tree.text() == null ||
                query.predicates[index.toInt()].all {
                    if (it !is QueryPredicate.Generic) it(match) else predicate(it, match)
                }
        }
    }
}
