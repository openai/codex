package io.github.treesitter.ktreesitter

/**
 * A function that is called while executing a query.
 *
 * The argument contains the current byte offset.
 *
 * If the function returns `false`, the execution will halt early.
 *
 * @since 0.25.0
 */
typealias QueryProgressCallback = (currentByteOffset: UInt) -> Boolean

/**
 * A class that represents a set of patterns which match nodes in a syntax tree.
 *
 * @constructor
 *  Create a new query from a particular language and a
 *  string containing one or more S-expression patterns.
 * @throws [QueryError] If any error occurred while creating the query.
 */
expect class Query @Throws(QueryError::class) constructor(language: Language, source: String) {
    /** The number of patterns in the query. */
    val patternCount: UInt

    /** The number of captures in the query. */
    @Deprecated("captureCount is deprecated.", ReplaceWith("captureNames.size"))
    val captureCount: UInt

    /**
     * The capture names used in the query.
     *
     * @since 0.25.0
     */
    val captureNames: List<String>

    /**
     * The string literals used in the query.
     *
     * @since 0.25.0
     */
    val stringValues: List<String>

    /**
     * Execute the query on the given [Node].
     *
     * @since 0.25.0
     */
    operator fun invoke(node: Node, progressCallback: QueryProgressCallback? = null): QueryCursor

    /**
     * Get the property settings for the given pattern index.
     *
     * Properties are set using the `#set!` predicate.
     *
     * @return A map of properties with optional values.
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [pattern count][patternCount].
     */
    fun settings(index: UInt): Map<String, String?>

    /**
     * Get the property assertions for the given pattern index.
     *
     * Assertions are performed using the `#is?` and `#is-not?` predicates.
     *
     * @return
     *  A map of assertions, where the first item is the optional property value
     *  and the second item indicates whether the assertion was positive or negative.
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [pattern count][patternCount].
     */
    @Throws(IndexOutOfBoundsException::class)
    fun assertions(index: UInt): Map<String, Pair<String?, Boolean>>

    /**
     * Disable a certain pattern within a query.
     *
     * This prevents the pattern from matching and removes most of the overhead
     * associated with the pattern. Currently, there is no way to undo this.
     *
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [pattern count][patternCount].
     */
    @Throws(IndexOutOfBoundsException::class)
    fun disablePattern(index: UInt)

    /**
     * Disable a certain capture within a query.
     *
     * This prevents the capture from being returned in matches,
     * and also avoids most resource usage associated with recording
     * the capture. Currently, there is no way to undo this.
     */
    fun disableCapture(name: String)

    /**
     * Get the byte offset where the given pattern starts in the query's source.
     *
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [pattern count][patternCount].
     */
    @Throws(IndexOutOfBoundsException::class)
    fun startByteForPattern(index: UInt): UInt

    /**
     * Get the byte offset where the given pattern ends in the query's source.
     *
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [pattern count][patternCount].
     * @since 0.23.0
     */
    @Throws(IndexOutOfBoundsException::class)
    fun endByteForPattern(index: UInt): UInt

    /**
     * Check if the pattern with the given index has a single root node.
     *
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [pattern count][patternCount].
     */
    @Throws(IndexOutOfBoundsException::class)
    fun isPatternRooted(index: UInt): Boolean

    /**
     * Check if the pattern with the given index is "non-local".
     *
     * A non-local pattern has multiple root nodes and can match within a
     * repeating sequence of nodes, as specified by the grammar. Non-local
     * patterns disable certain optimizations that would otherwise be possible
     * when executing a query on a specific range of a syntax tree.
     *
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [pattern count][patternCount].
     */
    @Throws(IndexOutOfBoundsException::class)
    fun isPatternNonLocal(index: UInt): Boolean

    /**
     * Check if a pattern is guaranteed to match once a given byte offset is reached.
     *
     * @throws [IndexOutOfBoundsException] If the offset exceeds the source length.
     */
    @Throws(IndexOutOfBoundsException::class)
    fun isPatternGuaranteedAtStep(offset: UInt): Boolean
}
