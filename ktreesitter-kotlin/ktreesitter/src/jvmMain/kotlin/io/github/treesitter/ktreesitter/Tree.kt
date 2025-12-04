package io.github.treesitter.ktreesitter

/** A class that represents a syntax tree. */
@Suppress("CanBeParameter")
actual class Tree internal constructor(
    private val self: Long,
    private var source: String?,
    /** The language that was used to parse the syntax tree. */
    actual val language: Language
) {
    init {
        RefCleaner(this, CleanAction(self))
    }

    /** The root node of the syntax tree. */
    actual val rootNode: Node
        external get

    /** The included ranges that were used to parse the syntax tree. */
    actual val includedRanges by lazy { nativeIncludedRanges() }

    /**
     * Get the root node of the syntax tree, but with
     * its position shifted forward by the given offset.
     */
    @JvmName("rootNodeWithOffset")
    actual external fun rootNodeWithOffset(bytes: UInt, extent: Point): Node?

    /**
     * Edit the syntax tree to keep it in sync
     * with source code that has been modified.
     */
    actual external fun edit(edit: InputEdit)

    /**
     * Create a shallow copy of the syntax tree.
     *
     * You need to copy a syntax tree in order to use it on multiple
     * threads or coroutines, as syntax trees are not thread safe.
     */
    actual fun copy() = Tree(copy(self), source, language)

    /** Create a new tree cursor starting from the node of the tree. */
    actual fun walk() = TreeCursor(rootNode)

    /** Get the source code of the syntax tree, if available. */
    actual fun text(): CharSequence? = source

    /**
     * Compare an old edited syntax tree to a new
     * syntax tree representing the same document.
     *
     * For this to work correctly, this tree must have been
     * edited such that its ranges match up to the new tree.
     *
     * @return A list of ranges whose syntactic structure has changed.
     */
    actual external fun changedRanges(newTree: Tree): List<Range>

    override fun toString() = "Tree(language=$language, source=$source)"

    private external fun nativeIncludedRanges(): List<Range>

    private class CleanAction(private val ptr: Long) : Runnable {
        override fun run() = delete(ptr)
    }

    private companion object {
        @JvmStatic
        private external fun copy(self: Long): Long

        @JvmStatic
        private external fun delete(self: Long)
    }
}
