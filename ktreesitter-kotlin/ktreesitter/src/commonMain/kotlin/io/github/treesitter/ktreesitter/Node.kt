package io.github.treesitter.ktreesitter

/** A single node within a [syntax tree][Tree]. */
expect class Node {
    /**
     * The numeric ID of the node.
     *
     * Within any given syntax tree, no two nodes have the same ID.
     * However, if a new tree is created based on an older tree,
     * and a node from the old tree is reused in the process,
     * then that node will have the same ID in both trees.
     */
    val id: ULong

    /** The numerical ID of the node's type. */
    val symbol: UShort

    /**
     * The numerical ID of the node's type,
     * as it appears in the grammar ignoring aliases.
     */
    val grammarSymbol: UShort

    /** The type of the node. */
    val type: String

    /**
     * The type of the node,
     * as it appears in the grammar ignoring aliases.
     */
    val grammarType: String

    /**
     * Check if the node is _named_.
     *
     * Named nodes correspond to named rules in the grammar,
     * whereas _anonymous_ nodes correspond to string literals.
     */
    val isNamed: Boolean

    /**
     * Check if the node is _extra_.
     *
     * Extra nodes represent things which are not required
     * by the grammar but can appear anywhere (e.g. whitespace).
     */
    val isExtra: Boolean

    /** Check if the node is a syntax error. */
    val isError: Boolean

    /**
     * Check if the node is _missing_.
     *
     * Missing nodes are inserted by the parser in order
     * to recover from certain kinds of syntax errors.
     */
    val isMissing: Boolean

    /** Check if the node has been edited. */
    val hasChanges: Boolean

    /**
     * Check if the node is a syntax error,
     * or contains any syntax errors.
     */
    val hasError: Boolean

    /** The parse state of this node. */
    val parseState: UShort

    /** The parse state after this node. */
    val nextParseState: UShort

    /** The start byte of the node. */
    val startByte: UInt

    /** The end byte of the node. */
    val endByte: UInt

    /** The range of the node in terms of bytes. */
    val byteRange: UIntRange

    /** The range of the node in terms of bytes and points. */
    val range: Range

    /** The start point of the node. */
    val startPoint: Point

    /** The end point of the node. */
    val endPoint: Point

    /** The number of this node's children. */
    val childCount: UInt

    /** The number of this node's _named_ children. */
    val namedChildCount: UInt

    /**
     * The number of this node's descendants,
     * including one for the node itself.
     */
    val descendantCount: UInt

    /** The node's immediate parent, if any. */
    val parent: Node?

    /** The node's next sibling, if any. */
    val nextSibling: Node?

    /** The node's previous sibling, if any. */
    val prevSibling: Node?

    /** The node's next _named_ sibling, if any. */
    val nextNamedSibling: Node?

    /** The node's previous _named_ sibling, if any. */
    val prevNamedSibling: Node?

    /**
     * This node's children.
     *
     * If you're walking the tree recursively,
     * you may want to use [walk] instead.
     */
    val children: List<Node>

    /** This node's _named_ children. */
    val namedChildren: List<Node>

    /**
     * The node's child at the given index, if any.
     *
     * This method is fairly fast, but its cost is technically
     * `log(i)`, so if you might be iterating over a long list of
     * children, you should use [children] or [Tree.walk] instead.
     *
     * @throws [IndexOutOfBoundsException]
     *  If the index exceeds the [child count][childCount].
     */
    @Throws(IndexOutOfBoundsException::class)
    fun child(index: UInt): Node?

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
    fun namedChild(index: UInt): Node?

    /**
     * Get the node's first child that contains
     * or starts after the given byte offset.
     *
     * @since 0.25.0
     */
    fun firstChildForByte(byte: UInt): Node?

    /**
     * Get the node's first _named_ child that contains
     * or starts after the given byte offset.
     *
     * @since 0.25.0
     */
    fun firstNamedChildForByte(byte: UInt): Node?

    /**
     * Get the node's child with the given field ID, if any.
     *
     * @see [Language.fieldIdForName]
     */
    fun childByFieldId(id: UShort): Node?

    /** Get the node's child with the given field name, if any. */
    fun childByFieldName(name: String): Node?

    /** Get a list of children with the given field ID. */
    fun childrenByFieldId(id: UShort): List<Node>

    /** Get a list of children with the given field name. */
    fun childrenByFieldName(name: String): List<Node>

    /**
     * Get the field name of this node’s child at the given index, if available.
     *
     * @throws [IndexOutOfBoundsException] If the index exceeds the [child count][childCount].
     */
    @Throws(IndexOutOfBoundsException::class)
    fun fieldNameForChild(index: UInt): String?

    /**
     * Get the field name of this node’s _named_ child at the given index, if available.
     *
     * @throws [IndexOutOfBoundsException] If the index exceeds the [child count][childCount].
     * @since 0.24.0
     */
    @Throws(IndexOutOfBoundsException::class)
    fun fieldNameForNamedChild(index: UInt): String?

    /**
     * Get the node that contains the given descendant, if any.
     *
     * @since 0.24.0
     */
    fun childWithDescendant(descendant: Node): Node?

    /**
     * Get the smallest node within this node
     * that spans the given byte range, if any.
     */
    fun descendant(start: UInt, end: UInt): Node?

    /**
     * Get the smallest node within this node
     * that spans the given point range, if any.
     */
    fun descendant(start: Point, end: Point): Node?

    /**
     * Get the smallest _named_ node within this node
     * that spans the given byte range, if any.
     */
    fun namedDescendant(start: UInt, end: UInt): Node?

    /**
     * Get the smallest _named_ node within this node
     * that spans the given point range, if any.
     */
    fun namedDescendant(start: Point, end: Point): Node?

    /**
     * Edit this node to keep it in-sync with source code that has been edited.
     *
     * This method is only rarely needed. When you edit a syntax tree via
     * [Tree.edit], all the nodes that you retrieve from the tree afterward
     * will already reflect the edit. You only need to use this when you have a
     * specific Node instance that you want to keep and continue to use after an edit.
     */
    fun edit(edit: InputEdit)

    /** Create a new tree cursor starting from this node. */
    fun walk(): TreeCursor

    /** Get the source code of the node, if available. */
    fun text(): CharSequence?

    /** Get the S-expression of the node. */
    fun sexp(): String

    override fun equals(other: Any?): Boolean

    override fun hashCode(): Int
}
