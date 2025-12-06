/**
 * Amounts by which to move a [Rect].
 *
 * Positive numbers move to the right/bottom and negative to the left/top.
 *
 * See [Rect.offset] for usage.
 */
package ratatui.layout


/**
 * Amounts by which to move a [Rect].
 *
 * @property x How much to move on the X axis
 * @property y How much to move on the Y axis
 */
data class Offset(
    val x: Int,
    val y: Int
) {

    companion object {
        /** A zero offset */
        val ZERO: Offset = Offset(0, 0)

        /** The minimum offset */
        val MIN: Offset = Offset(Int.MIN_VALUE, Int.MIN_VALUE)

        /** The maximum offset */
        val MAX: Offset = Offset(Int.MAX_VALUE, Int.MAX_VALUE)

        /** Creates a new Offset with the given values. */
        fun new(x: Int, y: Int): Offset = Offset(x, y)

        /** Create an Offset from a Position */
        fun from(position: Position): Offset = Offset(
            x = position.x.toInt(),
            y = position.y.toInt()
        )
    }
}
