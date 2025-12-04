package io.github.treesitter.ktreesitter

actual class Query @Throws(QueryError::class) actual constructor(
    private val language: Language,
    private val source: String
) {
    internal val self: Long = init(language.self, source)

    internal val predicates: List<MutableList<QueryPredicate>>

    private val settingList: List<MutableMap<String, String?>>

    private val assertionList: List<MutableMap<String, Pair<String?, Boolean>>>

    /** The number of patterns in the query. */
    @get:JvmName("getPatternCount")
    actual val patternCount: UInt
        get() = nativePatternCount().toUInt()

    /** The number of captures in the query. */
    @get:JvmName("getCaptureCount")
    @Deprecated("captureCount is deprecated.", ReplaceWith("captureNames.size"))
    actual val captureCount: UInt
        get() = nativeCaptureCount().toUInt()

    /**
     * The capture names used in the query.
     *
     * @since 0.25.0
     */
    actual val captureNames: List<String>

    /**
     * The string literals used in the query.
     *
     * @since 0.25.0
     */
    actual val stringValues: List<String>

    init {
        RefCleaner(this, CleanAction(self))

        predicates = List(nativePatternCount()) { mutableListOf() }
        settingList = List(nativePatternCount()) { mutableMapOf() }
        assertionList = List(nativePatternCount()) { mutableMapOf() }
        captureNames = List(nativeCaptureCount()) {
            checkNotNull(captureNameForId(it)) {
                "Failed to get capture name at index $it"
            }
        }
        stringValues = List(stringCount()) {
            checkNotNull(stringValueForId(it)) {
                "Failed to get string value at index $it"
            }
        }

        for (i in 0U..<patternCount) {
            val tokens = predicatesForPattern(i.toInt()) ?: continue
            val offset = startByteForPattern(i)
            val row = source.asSequence().withIndex()
                .takeWhile { it.index.toUInt() <= offset }
                .count { it.value == '\n' }.toUInt()
            var j = 0
            while (j < tokens.size) {
                var nargs = 0
                while (tokens[nargs].type != TSQueryPredicateStepTypeDone) ++nargs
                val t0 = tokens[j]
                if (t0.type == TSQueryPredicateStepTypeCapture) {
                    throw QueryError.Predicate(row, "@${captureNames[t0.value]}")
                }

                when (val pred = stringValues[t0.value]) {
                    "eq?", "not-eq?", "any-eq?", "any-not-eq?" -> {
                        if (nargs != 3) {
                            throw QueryError.Predicate(
                                row,
                                "#$pred expects 2 arguments, got ${nargs - 1}"
                            )
                        }
                        val t1 = tokens[j + 1]
                        if (t1.type != TSQueryPredicateStepTypeCapture) {
                            val value = stringValues[t1.value]
                            throw QueryError.Predicate(
                                row,
                                "first argument to #$pred must be a capture name, got \"$value\""
                            )
                        }
                        val t2 = tokens[j + 2]
                        val isPositive = pred == "eq?" || pred == "any-eq?"
                        val isAny = pred == "any-eq?" || pred == "any-not-eq?"
                        val value = if (t2.type == TSQueryPredicateStepTypeCapture) {
                            QueryPredicate.EqCapture(
                                pred,
                                captureNames[t1.value],
                                captureNames[t2.value],
                                isPositive,
                                isAny
                            )
                        } else {
                            QueryPredicate.EqString(
                                pred,
                                captureNames[t1.value],
                                stringValues[t2.value],
                                isPositive,
                                isAny
                            )
                        }
                        predicates[i] += value
                    }

                    "match?", "not-match?", "any-match?", "any-not-match?" -> {
                        if (nargs != 3) {
                            throw QueryError.Predicate(
                                row,
                                "#$pred expects 2 arguments, got ${nargs - 1}"
                            )
                        }
                        val t1 = tokens[j + 1]
                        if (t1.type != TSQueryPredicateStepTypeCapture) {
                            val value = stringValues[t1.value]
                            throw QueryError.Predicate(
                                row,
                                "first argument to #$pred must be a capture name, got \"$value\""
                            )
                        }
                        val t2 = tokens[j + 2]
                        if (t2.type != TSQueryPredicateStepTypeString) {
                            val value = captureNames[t1.value]
                            throw QueryError.Predicate(
                                row,
                                "second argument to #$pred must be a string literal, got @$value"
                            )
                        }
                        val pattern = try {
                            Regex(stringValues[t2.value])
                        } catch (cause: IllegalArgumentException) {
                            throw QueryError.Predicate(row, "pattern error", cause)
                        }
                        val value = QueryPredicate.Match(
                            pred,
                            captureNames[t1.value],
                            pattern,
                            pred == "match?" || pred == "any-match?",
                            pred == "any-match?" || pred == "any-not-match?"
                        )
                        predicates[i] += value
                    }

                    "any-of?", "not-any-of?" -> {
                        if (nargs < 3) {
                            throw QueryError.Predicate(
                                row,
                                "#$pred expects at least 2 arguments, got ${nargs - 1}"
                            )
                        }
                        val t1 = tokens[j + 1]
                        if (t1.type != TSQueryPredicateStepTypeCapture) {
                            val value = stringValues[t1.value]
                            throw QueryError.Predicate(
                                row,
                                "first argument to #$pred must be a capture name, got \"$value\""
                            )
                        }
                        val values = (2..<nargs).map {
                            val t = tokens[it]
                            if (t.type != TSQueryPredicateStepTypeString) {
                                val value = captureNames[t.value]
                                throw QueryError.Predicate(
                                    row,
                                    "arguments to #any-of? must be string literals, got @$value"
                                )
                            }
                            stringValues[t.value]
                        }
                        val value = QueryPredicate.AnyOf(
                            pred,
                            captureNames[t1.value],
                            values,
                            pred == "any-of?"
                        )
                        predicates[i] += value
                    }

                    "is?", "is-not?" -> {
                        if (nargs == 1 || nargs > 3) {
                            throw QueryError.Predicate(
                                row,
                                "#$pred expects 1-2 arguments, got ${nargs - 1}"
                            )
                        }
                        val t1 = tokens[j + 1]
                        if (t1.type != TSQueryPredicateStepTypeString) {
                            val value = captureNames[t1.value]
                            throw QueryError.Predicate(
                                row,
                                "first argument to #$pred must be a string literal, got @$value"
                            )
                        }
                        val key = stringValues[t1.value]
                        val value = if (nargs == 2) {
                            Pair(null, pred == "is?")
                        } else {
                            val t2 = tokens[j + 2]
                            if (t2.type != TSQueryPredicateStepTypeString) {
                                val value = captureNames[t2.value]
                                throw QueryError.Predicate(
                                    row,
                                    "second argument to #$pred must be a string literal, " +
                                        "got @$value"
                                )
                            }
                            Pair(stringValues[t2.value], pred == "is?")
                        }
                        assertionList[i][key] = value
                    }

                    "set!" -> {
                        if (nargs == 1 || nargs > 3) {
                            throw QueryError.Predicate(
                                row,
                                "#$pred expects 1-2 arguments, got ${nargs - 1}"
                            )
                        }
                        val t1 = tokens[j + 1]
                        if (t1.type != TSQueryPredicateStepTypeString) {
                            val value = captureNames[t1.value]
                            throw QueryError.Predicate(
                                row,
                                "first argument to #$pred must be a string literal, got @$value"
                            )
                        }
                        val key = stringValues[t1.value]
                        val value = if (nargs == 2) {
                            null
                        } else {
                            val t2 = tokens[j + 2]
                            if (t2.type != TSQueryPredicateStepTypeString) {
                                val value = captureNames[t2.value]
                                throw QueryError.Predicate(
                                    row,
                                    "second argument to #$pred must be a string literal, got @$value"
                                )
                            }
                            stringValues[t2.value]
                        }
                        settingList[i][key] = value
                    }

                    else -> {
                        val args = (1..<nargs).map {
                            val t = tokens[it]
                            if (t.type == TSQueryPredicateStepTypeString) {
                                QueryPredicateArg.Literal(stringValues[t.value])
                            } else {
                                QueryPredicateArg.Capture(captureNames[t.value])
                            }
                        }
                        predicates[i] += QueryPredicate.Generic(pred, args)
                    }
                }

                j += nargs + 1
            }
        }
    }

    /**
     * Execute the query on the given [Node].
     *
     * @since 0.25.0
     */
    @JvmOverloads
    @JvmName("exec")
    actual operator fun invoke(node: Node, progressCallback: QueryProgressCallback?) =
        QueryCursor(this, node, progressCallback)

    /**
     * Get the property settings for the given pattern index.
     *
     * Properties are set using the `#set!` predicate.
     *
     * @return A map of properties with optional values.
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [pattern count][patternCount].
     */
    @JvmName("settings")
    @Throws(IndexOutOfBoundsException::class)
    actual fun settings(index: UInt): Map<String, String?> {
        if (index >= patternCount)
            throw IndexOutOfBoundsException("Index $index exceeds count $patternCount")
        return settingList[index]
    }

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
    @JvmName("assertions")
    @Throws(IndexOutOfBoundsException::class)
    actual fun assertions(index: UInt): Map<String, Pair<String?, Boolean>> {
        if (index >= patternCount)
            throw IndexOutOfBoundsException("Index $index exceeds count $patternCount")
        return assertionList[index]
    }

    /**
     * Disable a certain pattern within a query.
     *
     * This prevents the pattern from matching and removes most of the overhead
     * associated with the pattern. Currently, there is no way to undo this.
     *
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [pattern count][patternCount].
     */
    @JvmName("disablePattern")
    @Throws(IndexOutOfBoundsException::class)
    actual external fun disablePattern(index: UInt)

    /**
     * Disable a certain capture within a query.
     *
     * This prevents the capture from being returned in matches,
     * and also avoids most resource usage associated with recording
     * the capture. Currently, there is no way to undo this.
     */
    actual external fun disableCapture(name: String)

    /**
     * Get the byte offset where the given pattern starts in the query's source.
     *
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [pattern count][patternCount].
     */
    @JvmName("startByteForPattern")
    @Throws(IndexOutOfBoundsException::class)
    actual external fun startByteForPattern(index: UInt): UInt

    /**
     * Get the byte offset where the given pattern ends in the query's source.
     *
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [pattern count][patternCount].
     * @since 0.23.0
     */
    @JvmName("endByteForPattern")
    @Throws(IndexOutOfBoundsException::class)
    actual external fun endByteForPattern(index: UInt): UInt

    /**
     * Check if the pattern with the given index has a single root node.
     *
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [pattern count][patternCount].
     */
    @JvmName("isPatternRooted")
    @Throws(IndexOutOfBoundsException::class)
    actual external fun isPatternRooted(index: UInt): Boolean

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
    @JvmName("isPatternNonLocal")
    @Throws(IndexOutOfBoundsException::class)
    actual external fun isPatternNonLocal(index: UInt): Boolean

    /**
     * Check if a pattern is guaranteed to match
     * once a given byte offset is reached.
     */
    @JvmName("isPatternGuaranteedAtStep")
    @Throws(IndexOutOfBoundsException::class)
    actual fun isPatternGuaranteedAtStep(offset: UInt): Boolean {
        if (offset >= source.length.toUInt())
            throw IndexOutOfBoundsException("Offset $offset exceeds EOF")
        return nativeIsPatternGuaranteedAtStep(offset.toInt())
    }

    override fun toString() = "Query(language=$language, source=$source)"

    private external fun nativePatternCount(): Int

    private external fun nativeCaptureCount(): Int

    private external fun stringCount(): Int

    private external fun captureNameForId(index: Int): String?

    private external fun stringValueForId(index: Int): String?

    private external fun nativeIsPatternGuaranteedAtStep(index: Int): Boolean

    private external fun predicatesForPattern(index: Int): List<IntArray>?

    @Suppress("NOTHING_TO_INLINE")
    private inline operator fun <T> List<T>.get(index: UInt) = get(index.toInt())

    private inline val IntArray.value: Int
        inline get() = get(0)

    private inline val IntArray.type: Int
        inline get() = get(1)

    private class CleanAction(private val ptr: Long) : Runnable {
        override fun run() = delete(ptr)
    }

    @Suppress("ConstPropertyName")
    private companion object {
        private const val TSQueryPredicateStepTypeDone = 0

        private const val TSQueryPredicateStepTypeCapture = 1

        private const val TSQueryPredicateStepTypeString = 2

        @JvmStatic
        @Throws(QueryError::class)
        private external fun init(language: Long, query: String): Long

        @JvmStatic
        private external fun delete(self: Long)
    }
}
