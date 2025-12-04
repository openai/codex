package io.github.treesitter.ktreesitter

import kotlin.jvm.JvmField
import kotlin.jvm.JvmName

/**
 * A position in a text document in terms of rows and columns.
 *
 * @property row The zero-based row of the document.
 * @property column The zero-based column of the document.
 */
data class Point(
    @get:JvmName("row") val row: UInt,
    @get:JvmName("column") val column: UInt
) : Comparable<Point> {
    override operator fun compareTo(other: Point): Int {
        val rowDiff = row.compareTo(other.row)
        if (rowDiff != 0) return rowDiff
        return column.compareTo(other.column)
    }

    companion object {
        /** The minimum value a [Point] can have. */
        @JvmField
        val MIN = Point(UInt.MIN_VALUE, UInt.MIN_VALUE)

        /** The maximum value a [Point] can have. */
        @JvmField
        val MAX = Point(UInt.MAX_VALUE, UInt.MAX_VALUE)
    }
}
