package io.github.treesitter.ktreesitter

/** A class that represents a syntax tree. */
expect class Tree {
    /** The root node of the syntax tree. */
    val rootNode: Node

    /** The language that was used to parse the syntax tree. */
    val language: Language

    /** The included ranges that were used to parse the syntax tree. */
    val includedRanges: List<Range>

    /**
     * Get the root node of the syntax tree, but with
     * its position shifted forward by the given offset.
     */
    fun rootNodeWithOffset(bytes: UInt, extent: Point): Node?

    /**
     * Edit the syntax tree to keep it in sync
     * with source code that has been modified.
     */
    fun edit(edit: InputEdit)

    /**
     * Create a shallow copy of the syntax tree.
     *
     * You need to copy a syntax tree in order to use it on multiple
     * threads or coroutines, as syntax trees are not thread safe.
     */
    fun copy(): Tree

    /** Create a new tree cursor starting from the node of the tree. */
    fun walk(): TreeCursor

    /** Get the source code of the syntax tree, if available. */
    fun text(): CharSequence?

    /**
     * Compare an old edited syntax tree to a new
     * syntax tree representing the same document.
     *
     * For this to work correctly, this tree must have been
     * edited such that its ranges match up to the new tree.
     *
     * @return A list of ranges whose syntactic structure has changed.
     */
    fun changedRanges(newTree: Tree): List<Range>
}
