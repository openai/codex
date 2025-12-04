package io.github.treesitter.ktreesitter

import io.github.treesitter.ktreesitter.internal.*
import kotlin.experimental.ExperimentalNativeApi
import kotlin.native.ref.createCleaner
import kotlinx.cinterop.*

/** A class that can be used to efficiently walk a [syntax tree][Tree]. */
@OptIn(ExperimentalForeignApi::class)
actual class TreeCursor private constructor(
    private val self: CPointer<TSTreeCursor>,
    internal actual val tree: Tree
) {
    internal constructor(node: Node) : this(ts_tree_cursor_new(node.self).ptr, node.tree) {
        internalNode = node
    }

    @Suppress("unused")
    @OptIn(ExperimentalNativeApi::class)
    private val cleaner = createCleaner(self) {
        ts_tree_cursor_delete(it)
        kts_free(it)
    }

    private var internalNode: Node? = null

    /** The current node of the cursor. */
    actual val currentNode: Node
        get() {
            if (internalNode == null)
                internalNode = ts_tree_cursor_current_node(self).convert(tree)
            return internalNode!!
        }

    /**
     * The depth of the cursor's current node relative to the
     * original node that the cursor was constructed with.
     */
    actual val currentDepth: UInt
        get() = ts_tree_cursor_current_depth(self)

    /**
     * The field ID of the tree cursor's current node, or `0`.
     *
     * @see [Node.childByFieldId]
     * @see [Language.fieldIdForName]
     */
    actual val currentFieldId: UShort
        get() = ts_tree_cursor_current_field_id(self)

    /**
     * The field name of the tree cursor's current node, if available.
     *
     * @see [Node.childByFieldName]
     */
    actual val currentFieldName: String?
        get() = ts_tree_cursor_current_field_name(self)?.toKString()

    /**
     * The index of the cursor's current node out of all the descendants
     * of the original node that the cursor was constructed with.
     */
    actual val currentDescendantIndex: UInt
        get() = ts_tree_cursor_current_descendant_index(self)

    /** Create a shallow copy of the tree cursor. */
    actual fun copy() = TreeCursor(ts_tree_cursor_copy(self).ptr, tree)

    /** Reset the cursor to start at a different node. */
    actual fun reset(node: Node) {
        ts_tree_cursor_reset(self, node.self)
        internalNode = null
    }

    /** Reset the cursor to start at the same position as another cursor. */
    actual fun reset(cursor: TreeCursor) {
        ts_tree_cursor_reset_to(self, cursor.self)
        internalNode = null
    }

    /**
     * Move the cursor to the first child of its current node.
     *
     * @return
     *  `true` if the cursor successfully moved,
     *  or `false` if there were no children.
     */
    actual fun gotoFirstChild(): Boolean {
        val result = ts_tree_cursor_goto_first_child(self)
        if (result) internalNode = null
        return result
    }

    /**
     * Move the cursor to the last child of its current node.
     *
     * @return
     *  `true` if the cursor successfully moved,
     *  or `false` if there were no children.
     */
    actual fun gotoLastChild(): Boolean {
        val result = ts_tree_cursor_goto_last_child(self)
        if (result) internalNode = null
        return result
    }

    /**
     * Move the cursor to the parent of its current node.
     *
     * @return
     *  `true` if the cursor successfully moved,
     *  or `false` if there was no parent node.
     */
    actual fun gotoParent(): Boolean {
        val result = ts_tree_cursor_goto_parent(self)
        if (result) internalNode = null
        return result
    }

    /**
     * Move the cursor to the next sibling of its current node.
     *
     * @return
     *  `true` if the cursor successfully moved,
     *  or `false` if there was no next sibling node.
     */
    actual fun gotoNextSibling(): Boolean {
        val result = ts_tree_cursor_goto_next_sibling(self)
        if (result) internalNode = null
        return result
    }

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
    actual fun gotoPreviousSibling(): Boolean {
        val result = ts_tree_cursor_goto_previous_sibling(self)
        if (result) internalNode = null
        return result
    }

    /**
     * Move the cursor to the node that is the nth descendant of
     * the original node that the cursor was constructed with,
     * where `0` represents the original node itself.
     */
    actual fun gotoDescendant(index: UInt) {
        ts_tree_cursor_goto_descendant(self, index)
        internalNode = null
    }

    /**
     * Move the cursor to the first child of its current
     * node that extends beyond the given byte offset.
     *
     * @return The index of the child node, or `null` if no such child was found.
     */
    actual fun gotoFirstChildForByte(byte: UInt): UInt? {
        val index = ts_tree_cursor_goto_first_child_for_byte(self, byte)
        if (index == -1L) return null
        internalNode = null
        return index.convert<UInt>()
    }

    /**
     * Move the cursor to the first child of its current
     * node that extends beyond the given point offset.
     *
     * @return The index of the child node, or `null` if no such child was found.
     */
    actual fun gotoFirstChildForPoint(point: Point): UInt? {
        val pointValue = cValue<TSPoint> { from(point) }
        val index = ts_tree_cursor_goto_first_child_for_point(self, pointValue)
        if (index == -1L) return null
        internalNode = null
        return index.convert<UInt>()
    }

    override fun toString() = "TreeCursor(tree=$tree)"
}
