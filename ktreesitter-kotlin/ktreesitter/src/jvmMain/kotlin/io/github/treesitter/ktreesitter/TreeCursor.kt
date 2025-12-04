package io.github.treesitter.ktreesitter

/** A class that can be used to efficiently walk a [syntax tree][Tree]. */
actual class TreeCursor private constructor(
    private val self: Long,
    @JvmField internal actual val tree: Tree
) {
    internal constructor(node: Node) : this(init(node), node.tree) {
        internalNode = node
    }

    init {
        RefCleaner(this, CleanAction(self))
    }

    @Suppress("unused")
    private var internalNode: Node? = null

    /** The current node of the cursor. */
    actual val currentNode: Node
        external get

    /**
     * The depth of the cursor's current node relative to the
     * original node that the cursor was constructed with.
     */
    @get:JvmName("getCurrentDepth")
    actual val currentDepth: UInt
        external get

    /**
     * The field ID of the tree cursor's current node, or `0`.
     *
     * @see [Node.childByFieldId]
     * @see [Language.fieldIdForName]
     */
    @get:JvmName("getCurrentFieldId")
    actual val currentFieldId: UShort
        external get

    /**
     * The field name of the tree cursor's current node, if available.
     *
     * @see [Node.childByFieldName]
     */
    actual val currentFieldName: String?
        external get

    /**
     * The index of the cursor's current node out of all the descendants
     * of the original node that the cursor was constructed with.
     */
    @get:JvmName("getCurrentDescendantIndex")
    actual val currentDescendantIndex: UInt
        external get

    /** Create a shallow copy of the tree cursor. */
    actual fun copy() = TreeCursor(copy(self), tree)

    /** Reset the cursor to start at a different node. */
    actual external fun reset(node: Node)

    /** Reset the cursor to start at the same position as another cursor. */
    actual external fun reset(cursor: TreeCursor)

    /**
     * Move the cursor to the first child of its current node.
     *
     * @return
     *  `true` if the cursor successfully moved,
     *  or `false` if there were no children.
     */
    actual external fun gotoFirstChild(): Boolean

    /**
     * Move the cursor to the last child of its current node.
     *
     * @return
     *  `true` if the cursor successfully moved,
     *  or `false` if there were no children.
     */
    actual external fun gotoLastChild(): Boolean

    /**
     * Move the cursor to the parent of its current node.
     *
     * @return
     *  `true` if the cursor successfully moved,
     *  or `false` if there was no parent node.
     */
    actual external fun gotoParent(): Boolean

    /**
     * Move the cursor to the next sibling of its current node.
     *
     * @return
     *  `true` if the cursor successfully moved,
     *  or `false` if there was no next sibling node.
     */
    actual external fun gotoNextSibling(): Boolean

    /**
     * Move the cursor to the previous sibling of its current node.
     *
     * This function may be slower than [gotoNextSibling] due to how node positions
     * are stored. In the worst case, this will need to iterate through all the
     * children up to the previous sibling node to recalculate its position.
     *
     * @return
     *  `true` if the cursor successfully moved,
     *  or `false` if there was no previous sibling node.
     */
    actual external fun gotoPreviousSibling(): Boolean

    /**
     * Move the cursor to the node that is the nth descendant of
     * the original node that the cursor was constructed with,
     * where `0` represents the original node itself.
     */
    @JvmName("gotoDescendant")
    actual external fun gotoDescendant(index: UInt)

    /**
     * Move the cursor to the first child of its current
     * node that extends beyond the given byte offset.
     *
     * @return The index of the child node, or `null` if no such child was found.
     */
    @JvmName("gotoFirstChildForByte")
    actual fun gotoFirstChildForByte(byte: UInt): UInt? {
        val result = nativeGotoFirstChildForByte(byte)
        if (result == -1L) return null
        internalNode = null
        return result.toUInt()
    }

    /**
     * Move the cursor to the first child of its current
     * node that extends beyond the given point offset.
     *
     * @return The index of the child node, or `null` if no such child was found.
     */
    @JvmName("gotoFirstChildForPoint")
    actual fun gotoFirstChildForPoint(point: Point): UInt? {
        val result = nativeGotoFirstChildForPoint(point)
        if (result == -1L) return null
        internalNode = null
        return result.toUInt()
    }

    override fun toString() = "TreeCursor(tree=$tree)"

    @JvmName("nativeGotoFirstChildForByte")
    private external fun nativeGotoFirstChildForByte(byte: UInt): Long

    @JvmName("nativeGotoFirstChildForPoint")
    private external fun nativeGotoFirstChildForPoint(point: Point): Long

    private class CleanAction(private val ptr: Long) : Runnable {
        override fun run() = delete(ptr)
    }

    private companion object {
        @JvmStatic
        private external fun init(node: Node): Long

        @JvmStatic
        private external fun copy(self: Long): Long

        @JvmStatic
        private external fun delete(self: Long)
    }
}
