package io.github.treesitter.ktreesitter

/** A class that can be used to efficiently walk a [syntax tree][Tree]. */
expect class TreeCursor {
    internal val tree: Tree

    /** The current node of the cursor. */
    val currentNode: Node

    /**
     * The depth of the cursor's current node relative to the
     * original node that the cursor was constructed with.
     */
    val currentDepth: UInt

    /**
     * The field ID of the tree cursor's current node, or `0`.
     *
     * @see [Node.childByFieldId]
     * @see [Language.fieldIdForName]
     */
    val currentFieldId: UShort

    /**
     * The field name of the tree cursor's current node, if available.
     *
     * @see [Node.childByFieldName]
     */
    val currentFieldName: String?

    /**
     * The index of the cursor's current node out of all the descendants
     * of the original node that the cursor was constructed with.
     */
    val currentDescendantIndex: UInt

    /** Create a shallow copy of the tree cursor. */
    fun copy(): TreeCursor

    /** Reset the cursor to start at a different node. */
    fun reset(node: Node)

    /** Reset the cursor to start at the same position as another cursor. */
    fun reset(cursor: TreeCursor)

    /**
     * Move the cursor to the first child of its current node.
     *
     * @return
     *  `true` if the cursor successfully moved,
     *  or `false` if there were no children.
     */
    fun gotoFirstChild(): Boolean

    /**
     * Move the cursor to the last child of its current node.
     *
     * @return
     *  `true` if the cursor successfully moved,
     *  or `false` if there were no children.
     */
    fun gotoLastChild(): Boolean

    /**
     * Move the cursor to the parent of its current node.
     *
     * @return
     *  `true` if the cursor successfully moved,
     *  or `false` if there was no parent node.
     */
    fun gotoParent(): Boolean

    /**
     * Move the cursor to the next sibling of its current node.
     *
     * @return
     *  `true` if the cursor successfully moved,
     *  or `false` if there was no next sibling node.
     */
    fun gotoNextSibling(): Boolean

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
    fun gotoPreviousSibling(): Boolean

    /**
     * Move the cursor to the node that is the nth descendant of
     * the original node that the cursor was constructed with,
     * where `0` represents the original node itself.
     */
    fun gotoDescendant(index: UInt)

    /**
     * Move the cursor to the first child of its current
     * node that extends beyond the given byte offset.
     *
     * @return The index of the child node, or `null` if no such child was found.
     */
    fun gotoFirstChildForByte(byte: UInt): UInt?

    /**
     * Move the cursor to the first child of its current
     * node that extends beyond the given point offset.
     *
     * @return The index of the child node, or `null` if no such child was found.
     */
    fun gotoFirstChildForPoint(point: Point): UInt?
}
