package io.github.treesitter.ktreesitter

/**
 * A class that is used to look up valid symbols in a specific parse state.
 *
 * Lookahead iterators can be useful to generate suggestions and improve syntax
 * error diagnostics. To get symbols valid in an `ERROR` node, use the lookahead
 * iterator on its first leaf node state. For `MISSING` nodes, a lookahead
 * iterator created on the previous non-extra leaf node may be appropriate.
 */
expect class LookaheadIterator : AbstractIterator<LookaheadIterator.Symbol> {
    /** The current language of the lookahead iterator. */
    val language: Language

    /**
     * The current symbol ID.
     *
     * The ID of the `ERROR` symbol is equal to `UShort.MAX_VALUE`.
     */
    val currentSymbol: UShort

    /**
     * The current symbol name.
     *
     * Newly created lookahead iterators will contain the `ERROR` symbol.
     */
    val currentSymbolName: String

    /**
     * Reset the lookahead iterator the given [state] and, optionally, another [language].
     *
     * @return `true` if the iterator was reset successfully or `false` if it failed.
     */
    fun reset(state: UShort, language: Language? = null): Boolean

    /** Advance the lookahead iterator to the next symbol. */
    override fun next(): Symbol

    /** Iterate over the symbol IDs. */
    fun symbols(): Sequence<UShort>

    /** Iterate over the symbol names. */
    fun symbolNames(): Sequence<String>

    override fun computeNext()

    /** A class that pairs a symbol ID with its name. */
    class Symbol(id: UShort, name: String) {
        val id: UShort
        val name: String

        operator fun component1(): UShort
        operator fun component2(): String
    }
}
