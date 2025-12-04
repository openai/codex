package io.github.treesitter.ktreesitter

import io.github.treesitter.ktreesitter.internal.*
import kotlinx.cinterop.*

/** A single node within a [syntax tree][Tree]. */
@OptIn(ExperimentalForeignApi::class)
actual class Node internal constructor(
    internal val self: CValue<TSNode>,
    internal val tree: Tree
) {
    /**
     * The numeric ID of the node.
     *
     * Within any given syntax tree, no two nodes have the same ID.
     * However, if a new tree is created based on an older tree,
     * and a node from the old tree is reused in the process,
     * then that node will have the same ID in both trees.
     */
    actual val id: ULong = kts_node_id(self)

    /** The numerical ID of the node's type. */
    actual val symbol: UShort
        get() = ts_node_symbol(self)

    /**
     * The numerical ID of the node's type,
     * as it appears in the grammar ignoring aliases.
     */
    actual val grammarSymbol: UShort
        get() = ts_node_grammar_symbol(self)

    /** The type of the node. */
    actual val type: String
        get() = ts_node_type(self)!!.toKString()

    /**
     * The type of the node,
     * as it appears in the grammar ignoring aliases.
     */
    actual val grammarType: String
        get() = ts_node_grammar_type(self)!!.toKString()

    /**
     * Check if the node is _named_.
     *
     * Named nodes correspond to named rules in the grammar,
     * whereas _anonymous_ nodes correspond to string literals.
     */
    actual val isNamed: Boolean
        get() = ts_node_is_named(self)

    /**
     * Check if the node is _extra_.
     *
     * Extra nodes represent things which are not required
     * by the grammar but can appear anywhere (e.g. whitespace).
     */
    actual val isExtra: Boolean
        get() = ts_node_is_extra(self)

    /** Check if the node is a syntax error. */
    actual val isError: Boolean
        get() = ts_node_is_error(self)

    /**
     * Check if the node is _missing_.
     *
     * Missing nodes are inserted by the parser in order
     * to recover from certain kinds of syntax errors.
     */
    actual val isMissing: Boolean
        get() = ts_node_is_missing(self)

    /** Check if the node has been edited. */
    actual val hasChanges: Boolean
        get() = ts_node_has_changes(self)

    /**
     * Check if the node is a syntax error,
     * or contains any syntax errors.
     */
    actual val hasError: Boolean
        get() = ts_node_has_error(self)

    /** The parse state of this node. */
    actual val parseState: UShort
        get() = ts_node_parse_state(self)

    /** The parse state after this node. */
    actual val nextParseState: UShort
        get() = ts_node_next_parse_state(self)

    /** The start byte of the node. */
    actual val startByte: UInt
        get() = ts_node_start_byte(self)

    /** The end byte of the node. */
    actual val endByte: UInt
        get() = ts_node_end_byte(self)

    /** The range of the node in terms of bytes. */
    actual val byteRange: UIntRange
        get() = startByte..endByte

    /** The range of the node in terms of bytes and points. */
    actual val range: Range
        get() = Range(startPoint, endPoint, startByte, endByte)

    /** The start point of the node. */
    actual val startPoint: Point
        get() = ts_node_start_point(self).useContents(TSPoint::convert)

    /** The end point of the node. */
    actual val endPoint: Point
        get() = ts_node_end_point(self).useContents(TSPoint::convert)

    /** The number of this node's children. */
    actual val childCount: UInt
        get() = ts_node_child_count(self)

    /** The number of this node's _named_ children. */
    actual val namedChildCount: UInt
        get() = ts_node_named_child_count(self)

    /**
     * The number of this node's descendants,
     * including one for the node itself.
     */
    actual val descendantCount: UInt
        get() = ts_node_descendant_count(self)

    /** The node's immediate parent, if any. */
    actual val parent: Node?
        get() = ts_node_parent(self).convert(tree)

    /** The node's next sibling, if any. */
    actual val nextSibling: Node?
        get() = ts_node_next_sibling(self).convert(tree)

    /** The node's previous sibling, if any. */
    actual val prevSibling: Node?
        get() = ts_node_prev_sibling(self).convert(tree)

    /** The node's next _named_ sibling, if any. */
    actual val nextNamedSibling: Node?
        get() = ts_node_next_named_sibling(self).convert(tree)

    /** The node's previous _named_ sibling, if any. */
    actual val prevNamedSibling: Node?
        get() = ts_node_prev_named_sibling(self).convert(tree)

    private var internalChildren: List<Node>? = null

    /**
     * This node's children.
     *
     * If you're walking the tree recursively,
     * you may want to use [walk] instead.
     */
    actual val children: List<Node>
        get() {
            if (internalChildren == null) {
                val length = childCount.toInt()
                if (length == 0) return emptyList()
                val cursor = ts_tree_cursor_new(self).ptr
                ts_tree_cursor_goto_first_child(cursor)
                internalChildren = List(length) {
                    val node = ts_tree_cursor_current_node(cursor)
                    ts_tree_cursor_goto_next_sibling(cursor)
                    Node(node, tree)
                }
                ts_tree_cursor_delete(cursor)
                kts_free(cursor)
            }
            return internalChildren!!
        }

    /** This node's _named_ children. */
    actual val namedChildren: List<Node>
        get() = children.filter(Node::isNamed)

    /**
     * The node's child at the given index, if any.
     *
     * This method is fairly fast, but its cost is technically `log(i)`,
     * so if you might be iterating over a long list of children,
     * you should use [children] or [walk][Node.walk] instead.
     *
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [child count][childCount].
     */
    @Throws(IndexOutOfBoundsException::class)
    actual fun child(index: UInt): Node? {
        if (index < childCount) return ts_node_child(self, index).convert(tree)
        throw IndexOutOfBoundsException("Child index $index is out of bounds")
    }

    /**
     * Get the node's _named_ child at the given index, if any.
     *
     * This method is fairly fast, but its cost is technically `log(i)`,
     * so if you might be iterating over a long list of children,
     * you should use [namedChildren] or [walk][Node.walk] instead.
     *
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [named child count][namedChildCount].
     */
    @Throws(IndexOutOfBoundsException::class)
    actual fun namedChild(index: UInt): Node? {
        if (index < namedChildCount) return ts_node_named_child(self, index).convert(tree)
        throw IndexOutOfBoundsException("Child index $index is out of bounds")
    }

    /**
     * Get the node's first child that contains
     * or starts after the given byte offset.
     *
     * @since 0.25.0
     */
    actual fun firstChildForByte(byte: UInt) =
        ts_node_first_child_for_byte(self, byte).convert(tree)

    /**
     * Get the node's first _named_ child that contains
     * or starts after the given byte offset.
     *
     * @since 0.25.0
     */
    actual fun firstNamedChildForByte(byte: UInt) =
        ts_node_first_named_child_for_byte(self, byte).convert(tree)

    /**
     * Get the node's child with the given field ID, if any.
     *
     * @see [Language.fieldIdForName]
     */
    actual fun childByFieldId(id: UShort) = ts_node_child_by_field_id(self, id).convert(tree)

    /** Get the node's child with the given field name, if any. */
    actual fun childByFieldName(name: String) =
        ts_node_child_by_field_name(self, name, name.length.convert()).convert(tree)

    /** Get a list of children with the given field ID. */
    actual fun childrenByFieldId(id: UShort): List<Node> {
        if (id == UShort.MIN_VALUE) return emptyList()
        val length = childCount.toInt()
        if (length == 0) return emptyList()
        val children = ArrayList<Node>(length)
        val cursor = ts_tree_cursor_new(self).ptr
        var ok = ts_tree_cursor_goto_first_child(cursor)
        while (ok) {
            if (ts_tree_cursor_current_field_id(cursor) == id)
                children += Node(ts_tree_cursor_current_node(cursor), tree)
            ok = ts_tree_cursor_goto_next_sibling(cursor)
        }
        ts_tree_cursor_delete(cursor)
        kts_free(cursor)
        return children.apply { trimToSize() }
    }

    /** Get a list of children with the given field name. */
    actual fun childrenByFieldName(name: String) =
        childrenByFieldId(tree.language.fieldIdForName(name))

    /**
     * Get the field name of this node’s child at the given index, if available.
     *
     * @throws [IndexOutOfBoundsException] If the index exceeds the [child count][childCount].
     */
    @Throws(IndexOutOfBoundsException::class)
    actual fun fieldNameForChild(index: UInt): String? {
        if (index < childCount) return ts_node_field_name_for_child(self, index)?.toKString()
        throw IndexOutOfBoundsException("Child index $index is out of bounds")
    }

    /**
     * Get the field name of this node’s _named_ child at the given index, if available.
     *
     * @throws [IndexOutOfBoundsException] If the index exceeds the [child count][childCount].
     * @since 0.24.0
     */
    @Throws(IndexOutOfBoundsException::class)
    actual fun fieldNameForNamedChild(index: UInt): String? {
        if (index < childCount) return ts_node_field_name_for_named_child(self, index)?.toKString()
        throw IndexOutOfBoundsException("Child index $index is out of bounds")
    }

    /**
     * Get the node that contains the given descendant, if any.
     *
     * @since 0.24.0
     */
    actual fun childWithDescendant(descendant: Node) =
        ts_node_child_with_descendant(self, descendant.self).convert(tree)

    /**
     * Get the smallest node within this node
     * that spans the given byte range, if any.
     */
    actual fun descendant(start: UInt, end: UInt) =
        ts_node_descendant_for_byte_range(self, start, end).convert(tree)

    /**
     * Get the smallest node within this node
     * that spans the given point range, if any.
     */
    actual fun descendant(start: Point, end: Point): Node? {
        val startPoint = cValue<TSPoint> { from(start) }
        val endPoint = cValue<TSPoint> { from(end) }
        return ts_node_descendant_for_point_range(self, startPoint, endPoint).convert(tree)
    }

    /**
     * Get the smallest _named_ node within this node
     * that spans the given byte range, if any.
     */
    actual fun namedDescendant(start: UInt, end: UInt) =
        ts_node_named_descendant_for_byte_range(self, start, end).convert(tree)

    /**
     * Get the smallest _named_ node within this node
     * that spans the given point range, if any.
     */
    actual fun namedDescendant(start: Point, end: Point): Node? {
        val startPoint = cValue<TSPoint> { from(start) }
        val endPoint = cValue<TSPoint> { from(end) }
        return ts_node_named_descendant_for_point_range(self, startPoint, endPoint).convert(tree)
    }

    /**
     * Edit this node to keep it in-sync with source code that has been edited.
     *
     * This method is only rarely needed. When you edit a syntax tree via
     * [Tree.edit], all the nodes that you retrieve from the tree afterward
     * will already reflect the edit. You only need to use this when you have a
     * specific Node instance that you want to keep and continue to use after an edit.
     */
    @ExperimentalMultiplatform
    actual fun edit(edit: InputEdit) {
        val inputEdit = cValue<TSInputEdit> { from(edit) }
        val arena = Arena()
        val node = interpretCPointer<TSNode>(
            arena.alloc(self.size, self.align).rawPtr
        )
        ts_node_edit(node, inputEdit)
        internalChildren = null
        arena.clear()
    }

    /** Create a new tree cursor starting from this node. */
    actual fun walk() = TreeCursor(this)

    /** Get the source code of the node, if available. */
    actual fun text() = tree.text()?.run {
        subSequence(startByte.toInt(), minOf(endByte.toInt(), length))
    }

    /** Get the S-expression of the node. */
    actual fun sexp(): String {
        val string = ts_node_string(self)
        val result = string?.toKString() ?: ""
        kts_free(string)
        return result
    }

    actual override fun equals(other: Any?) =
        this === other || (other is Node && ts_node_eq(self, other.self))

    actual override fun hashCode(): Int = kts_node_hash(self)

    override fun toString() = "Node(type=$type, startByte=$startByte, endByte=$endByte)"
}
