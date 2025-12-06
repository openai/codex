/**
 * Amounts by which to move a [Rect].
 *
 * Positive numbers move to the right/bottom and negative to the left/top.
 *
 * See [Rect.offset] for usage.
 */
package ai.solace.coder.tui.layout

import kotlin.test.Test
import kotlin.test.assertEquals

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

// Tests
class OffsetTest {

    @Test
    fun testNewSetsComponents() {
        assertEquals(Offset.new(-3, 7), Offset(x = -3, y = 7))
    }

    @Test
    fun testConstantsMatchExpectedValues() {
        assertEquals(Offset.ZERO, Offset.new(0, 0))
        assertEquals(Offset.MIN, Offset.new(Int.MIN_VALUE, Int.MIN_VALUE))
        assertEquals(Offset.MAX, Offset.new(Int.MAX_VALUE, Int.MAX_VALUE))
    }

    @Test
    fun testFromPositionConvertsCoordinates() {
        val position = Position.new(4u, 9u)
        val offset = Offset.from(position)

        assertEquals(offset, Offset.new(4, 9))
    }
}
