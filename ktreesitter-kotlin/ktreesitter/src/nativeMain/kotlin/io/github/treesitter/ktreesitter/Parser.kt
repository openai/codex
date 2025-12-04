package io.github.treesitter.ktreesitter

import io.github.treesitter.ktreesitter.internal.*
import kotlin.experimental.ExperimentalNativeApi
import kotlin.native.ref.createCleaner
import kotlinx.cinterop.*

/**
 * A class that is used to produce a [syntax tree][Tree] from source code.
 *
 * @constructor Create a new instance with a certain [language], or `null` if empty.
 */
@OptIn(ExperimentalForeignApi::class)
actual class Parser actual constructor() {
    actual constructor(language: Language) : this() {
        this.language = language
    }

    private val self = ts_parser_new()

    @Suppress("unused")
    @OptIn(ExperimentalNativeApi::class)
    private val cleaner = createCleaner(self) {
        freeLogger(ts_parser_logger(it))
        ts_parser_delete(it)
    }

    /**
     * The language that the parser will use for parsing.
     *
     * Parsing cannot be performed while the language is `null`.
     */
    actual var language: Language? = null
        set(value) {
            ts_parser_set_language(self, value?.self)
            field = value
        }

    /**
     * The ranges of text that the parser should include when parsing.
     *
     * By default, the parser will always include entire documents.
     * Setting this property allows you to parse only a _portion_ of a
     * document but still return a syntax tree whose ranges match up with
     * the document as a whole. You can also pass multiple disjoint ranges.
     *
     * @throws [IllegalArgumentException] If the ranges overlap or are not in ascending order.
     */
    actual var includedRanges: List<Range> = emptyList()
        set(value) {
            val size = value.size
            if (size > 0) {
                val arena = Arena()
                val ranges = arena.allocArray<TSRange>(size) {
                    arena.alloc<TSRange>().from(value[it])
                }
                val result = ts_parser_set_included_ranges(self, ranges, size.convert())
                arena.clear()
                require(result) {
                    "Included ranges must be in ascending order and must not overlap"
                }
            } else {
                ts_parser_set_included_ranges(self, null, 0U)
            }
            field = value
        }

    /**
     * The maximum duration in microseconds that parsing
     * should be allowed to take before halting.
     */
    @Deprecated("Use the progressCallback in parse()")
    actual var timeoutMicros: ULong
        get() = ts_parser_timeout_micros(self)
        set(value) = ts_parser_set_timeout_micros(self, value)

    /** The logger that the parser will use during parsing. */
    @get:Deprecated("The logger can't be called directly.", level = DeprecationLevel.HIDDEN)
    actual var logger: LogFunction? = null
        set(value) {
            if (field != null)
                freeLogger(ts_parser_logger(self))
            val logger = cValue<TSLogger> {
                if (value != null) {
                    payload = StableRef.create(value).asCPointer()
                    log = staticCFunction { payload, type, message ->
                        val callback = payload?.asStableRef<LogFunction>()?.get()
                        if (callback != null && message != null)
                            callback(LogType.entries[type.ordinal], message.toKString())
                    }
                } else {
                    payload = null
                    log = null
                }
            }
            ts_parser_set_logger(self, logger)
            field = value
        }

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
    actual fun parse(source: String, encoding: InputEncoding, oldTree: Tree?): Tree {
        val language = checkNotNull(language) {
            "The parser has no language assigned"
        }
        val tree = ts_parser_parse_string_encoding(
            self,
            oldTree?.self,
            source,
            source.length.convert(),
            encoding.value
        )
        checkNotNull(tree) { "Parsing failed" }
        return Tree(tree, source, language)
    }

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
    actual fun parse(
        encoding: InputEncoding,
        oldTree: Tree?,
        progressCallback: ParseProgressCallback?,
        readCallback: ParseReadCallback
    ): Tree {
        val language = checkNotNull(language) {
            "The parser has no language assigned"
        }
        val arena = Arena()
        val payloadRef = StableRef.create(ParsePayload(arena, readCallback))
        val input = cValue<TSInput> {
            payload = payloadRef.asCPointer()
            this.encoding = encoding.value
            read = staticCFunction { payload, index, point, bytes ->
                val data = payload!!.asStableRef<ParsePayload>().get()
                val result = data.callback(index, point.useContents { convert() })
                bytes!!.pointed.value = result?.length?.convert() ?: 0U
                result?.toString()?.cstr?.getPointer(data.memScope)
            }
        }
        var progressRef: StableRef<ParseProgressCallback>? = null
        val tree = if (progressCallback == null) {
            ts_parser_parse(self, oldTree?.self, input)
        } else {
            progressRef = StableRef.create(progressCallback)
            val options = cValue<TSParseOptions> {
                payload = progressRef.asCPointer()
                progress_callback = staticCFunction { state ->
                    val callback = state!!.pointed.payload!!
                        .asStableRef<ParseProgressCallback>().get()
                    callback(state.pointed.current_byte_offset, state.pointed.has_error)
                }
            }
            ts_parser_parse_with_options(self, oldTree?.self, input, options)
        }
        arena.clear()
        payloadRef.dispose()
        progressRef?.dispose()
        checkNotNull(tree) { "Parsing failed" }
        return Tree(tree, null, language)
    }

    /**
     * Instruct the parser to start the next [parse] from the beginning.
     *
     * If parsing was previously halted, then by default, it will resume where
     * it left off. If you don't want to resume, and instead intend to use this
     * parser to parse some other document, you must call this method first.
     */
    actual fun reset() = ts_parser_reset(self)

    override fun toString() = "Parser(language=$language)"

    /** The type of a log message. */
    actual enum class LogType { LEX, PARSE }

    private class ParsePayload(
        val memScope: AutofreeScope,
        val callback: ParseReadCallback
    )

    private companion object {
        private fun freeLogger(logger: CValue<TSLogger>) {
            val arena = Arena()
            interpretNullablePointed<TSLogger>(
                arena.alloc(logger.size, logger.align).rawPtr
            )?.payload?.asStableRef<TSLogger>()?.dispose()
            arena.clear()
        }
    }
}
