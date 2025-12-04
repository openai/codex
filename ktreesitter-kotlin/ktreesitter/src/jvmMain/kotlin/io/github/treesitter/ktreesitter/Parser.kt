package io.github.treesitter.ktreesitter

/**
 * A class that is used to produce a [syntax tree][Tree] from source code.
 *
 * @constructor Create a new instance with a certain [language], or `null` if empty.
 */
actual class Parser actual constructor() {
    actual constructor(language: Language) : this() {
        this.language = language
    }

    private val self = init()

    init {
        RefCleaner(this, CleanAction(self))
    }

    /**
     * The language that the parser will use for parsing.
     *
     * Parsing cannot be performed while the language is `null`.
     */
    actual var language: Language? = null
        external set

    /**
     * The ranges of text that the parser will include when parsing.
     *
     * By default, the parser will always include entire documents.
     * Setting this property allows you to parse only a _portion_ of a
     * document but still return a syntax tree whose ranges match up with
     * the document as a whole. You can also pass multiple disjoint ranges.
     *
     * @throws [IllegalArgumentException] If the ranges overlap or are not in ascending order.
     */
    @set:Throws(IllegalArgumentException::class)
    actual var includedRanges: List<Range> = emptyList()
        external set

    /**
     * The maximum duration in microseconds that parsing
     * should be allowed to take before halting.
     */
    @get:JvmName("getTimeoutMicros")
    @set:JvmName("setTimeoutMicros")
    @Deprecated("Use the progressCallback in parse()")
    actual var timeoutMicros: ULong
        external get
        external set

    /**
     * The logger that the parser will use during parsing.
     *
     * #### Example
     *
     * ```
     * import org.slf4j.LoggerFactory;
     * import org.slf4j.MarkerFactory;
     *
     * val logger = LoggerFactory.getLogger(parser.javaClass)
     * val lexMarker = MarkerFactory.getMarker("TS LEX")
     * val parseMarker = MarkerFactory.getMarker("TS PARSE")
     *
     * parser.logger = { type, msg ->
     *     val marker = when (type) {
     *         LogType.LEX -> lexMarker
     *         LogType.PARSE -> parseMarker
     *     }
     *     logger.debug(marker, msg)
     * }
     * ```
     */
    @get:Deprecated("Don't call the logger directly.", level = DeprecationLevel.HIDDEN)
    actual var logger: LogFunction? = null
        external set

    /**
     * Parse a source code string and create a syntax tree.
     *
     * If you have already parsed an earlier version of this document and the document
     * has since been edited, pass the previous syntax tree to [oldTree] so that the
     * unchanged parts of it can be reused. This will save time and memory. For this
     * to work correctly, you must have already edited the old syntax tree using the
     * [Tree.edit] method in a way that exactly matches the source code changes.
     *
     * @throws [IllegalStateException]
     *  If the parser does not have a [language] assigned or if parsing was halted.
     */
    @Throws(IllegalStateException::class)
    actual external fun parse(source: String, encoding: InputEncoding, oldTree: Tree?): Tree

    /**
     * Parse source code from a callback and create a syntax tree.
     *
     * If you have already parsed an earlier version of this document and the document
     * has since been edited, pass the previous syntax tree to [oldTree] so that the
     * unchanged parts of it can be reused. This will save time and memory. For this
     * to work correctly, you must have already edited the old syntax tree using the
     * [Tree.edit] method in a way that exactly matches the source code changes.
     *
     * @throws [IllegalStateException]
     *  If the parser does not have a [language] assigned or if parsing was halted.
     */
    @Throws(IllegalStateException::class)
    actual external fun parse(
        encoding: InputEncoding,
        oldTree: Tree?,
        progressCallback: ParseProgressCallback?,
        readCallback: ParseReadCallback
    ): Tree

    /**
     * Instruct the parser to start the next [parse] from the beginning.
     *
     * If parsing was previously halted, then by default, it will resume where
     * it left off. If you don't want to resume, and instead intend to use this
     * parser to parse some other document, you must call this method first.
     */
    actual external fun reset()

    override fun toString() = "Parser(language=$language)"

    /** The type of a log message. */
    @Suppress("unused")
    actual enum class LogType { LEX, PARSE }

    private class CleanAction(private val ptr: Long) : Runnable {
        override fun run() = delete(ptr)
    }

    private companion object {
        @JvmStatic
        private external fun init(): Long

        @JvmStatic
        private external fun delete(self: Long)

        init {
            NativeUtils.loadLibrary()
        }
    }
}
