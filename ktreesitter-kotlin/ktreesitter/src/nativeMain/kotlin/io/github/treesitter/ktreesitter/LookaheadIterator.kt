package io.github.treesitter.ktreesitter

import io.github.treesitter.ktreesitter.internal.*
import kotlin.experimental.ExperimentalNativeApi
import kotlin.native.ref.createCleaner
import kotlinx.cinterop.*

/**
 * A class that is used to look up valid symbols in a specific parse state.
 *
 * Lookahead iterators can be useful to generate suggestions and improve syntax
 * error diagnostics. To get symbols valid in an `ERROR` node, use the lookahead
 * iterator on its first leaf node state. For `MISSING` nodes, a lookahead
 * iterator created on the previous non-extra leaf node may be appropriate.
 */
@OptIn(ExperimentalForeignApi::class)
actual class LookaheadIterator @Throws(IllegalArgumentException::class) internal constructor(
    language: Language,
    private val state: UShort
) : AbstractIterator<LookaheadIterator.Symbol>() {
    private val self = ts_lookahead_iterator_new(language.self, state)
        ?: throw IllegalArgumentException("State $state is not valid for $language")

    /** The current language of the lookahead iterator. */
    actual val language: Language
        get() = Language(ts_lookahead_iterator_language(self)!!)

    @Suppress("unused")
    @OptIn(ExperimentalNativeApi::class)
    private val cleaner = createCleaner(self, ::ts_lookahead_iterator_delete)

    /**
     * The current symbol ID.
     *
     * The ID of the `ERROR` symbol is equal to `UShort.MAX_VALUE`.
     */
    actual val currentSymbol: UShort
        get() = ts_lookahead_iterator_current_symbol(self)

    /**
     * The current symbol name.
     *
     * Newly created lookahead iterators will contain the `ERROR` symbol.
     */
    actual val currentSymbolName: String
        get() = ts_lookahead_iterator_current_symbol_name(self)!!.toKString()

    /**
     * Reset the lookahead iterator the given [state] and, optionally, another [language].
     *
     * @return `true` if the iterator was reset successfully or `false` if it failed.
     */
    actual fun reset(state: UShort, language: Language?): Boolean = if (language == null) {
        ts_lookahead_iterator_reset_state(self, state)
    } else {
        ts_lookahead_iterator_reset(self, language.self, state)
    }

    /** Advance the lookahead iterator to the next symbol. */
    actual override fun next() = super.next()

    /** Iterate over the symbol IDs. */
    actual fun symbols(): Sequence<UShort> {
        ts_lookahead_iterator_reset_state(self, state)
        return sequence {
            while (ts_lookahead_iterator_next(self)) {
                yield(ts_lookahead_iterator_current_symbol(self))
            }
        }
    }

    /** Iterate over the symbol names. */
    actual fun symbolNames(): Sequence<String> {
        ts_lookahead_iterator_reset_state(self, state)
        return sequence {
            while (ts_lookahead_iterator_next(self)) {
                yield(ts_lookahead_iterator_current_symbol_name(self)!!.toKString())
            }
        }
    }

    actual override fun computeNext() = if (ts_lookahead_iterator_next(self)) {
        val id = ts_lookahead_iterator_current_symbol(self)
        val name = ts_lookahead_iterator_current_symbol_name(self)
        setNext(Symbol(id, name!!.toKString()))
    } else {
        done()
    }

    operator fun iterator() = apply { reset(state) }

    /** A class that pairs a symbol ID with its name. */
    actual data class Symbol actual constructor(actual val id: UShort, actual val name: String)
}
