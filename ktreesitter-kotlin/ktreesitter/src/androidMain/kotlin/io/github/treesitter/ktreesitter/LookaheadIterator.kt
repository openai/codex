package io.github.treesitter.ktreesitter

import dalvik.annotation.optimization.CriticalNative
import dalvik.annotation.optimization.FastNative

/**
 * A class that is used to look up valid symbols in a specific parse state.
 *
 * Lookahead iterators can be useful to generate suggestions and improve syntax
 * error diagnostics. To get symbols valid in an `ERROR` node, use the lookahead
 * iterator on its first leaf node state. For `MISSING` nodes, a lookahead
 * iterator created on the previous non-extra leaf node may be appropriate.
 *
 * __NOTE:__ If you're targeting Android SDK level < 33,
 * you must `use` or [close] the instance to free up resources.
 */
actual class LookaheadIterator @Throws(IllegalArgumentException::class) internal constructor(
    language: Language,
    private val state: UShort
) : AbstractIterator<LookaheadIterator.Symbol>(), AutoCloseable {
    private val self: Long = init(language.self, state).takeIf { it > 0L }
        ?: throw IllegalArgumentException("State $state is not valid for $language")

    init {
        RefCleaner(this, CleanAction(self))
    }

    /** The current language of the lookahead iterator. */
    actual val language: Language
        @FastNative external get

    /**
     * The current symbol ID.
     *
     * The ID of the `ERROR` symbol is equal to `UShort.MAX_VALUE`.
     */
    @get:JvmName("getCurrentSymbol")
    actual val currentSymbol: UShort
        @FastNative external get

    /**
     * The current symbol name.
     *
     * Newly created lookahead iterators will contain the `ERROR` symbol.
     */
    actual val currentSymbolName: String
        @FastNative external get

    /**
     * Reset the lookahead iterator the given [state] and, optionally, another [language].
     *
     * @return `true` if the iterator was reset successfully or `false` if it failed.
     */
    @FastNative
    @JvmName("reset")
    actual external fun reset(state: UShort, language: Language?): Boolean

    /** Advance the lookahead iterator to the next symbol. */
    actual override fun next() = super.next()

    /** Iterate over the symbol IDs. */
    actual fun symbols(): Sequence<UShort> {
        reset(state)
        return sequence {
            while (nativeNext()) {
                yield(currentSymbol)
            }
        }
    }

    /** Iterate over the symbol names. */
    actual fun symbolNames(): Sequence<String> {
        reset(state)
        return sequence {
            while (nativeNext()) {
                yield(currentSymbolName)
            }
        }
    }

    override fun close() = delete(self)

    actual override fun computeNext() = if (nativeNext()) {
        setNext(Symbol(currentSymbol, currentSymbolName))
    } else {
        done()
    }

    operator fun iterator() = apply { reset(state) }

    @FastNative
    private external fun nativeNext(): Boolean

    /** A class that pairs a symbol ID with its name. */
    @JvmRecord
    actual data class Symbol actual constructor(actual val id: UShort, actual val name: String)

    private class CleanAction(private val ptr: Long) : Runnable {
        override fun run() = delete(ptr)
    }

    private companion object {
        @JvmStatic
        @JvmName("init")
        @CriticalNative
        private external fun init(language: Long, state: UShort): Long

        @JvmStatic
        @CriticalNative
        private external fun delete(self: Long)
    }
}
