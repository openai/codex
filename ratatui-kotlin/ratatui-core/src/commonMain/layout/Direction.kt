/**
 * Defines the direction of a layout.
 *
 * This enumeration is used with [Layout] to specify whether layout
 * segments should be arranged horizontally or vertically.
 *
 * - [Horizontal]: Layout segments are arranged side by side (left to right)
 * - [Vertical]: Layout segments are arranged top to bottom (default)
 *
 * For comprehensive layout documentation and examples, see the layout module.
 */
package ai.solace.coder.tui.layout

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/**
 * Defines the direction of a layout.
 */
enum class Direction {
    /** Layout segments are arranged side by side (left to right). */
    Horizontal,

    /** Layout segments are arranged top to bottom (default). */
    Vertical;

    /**
     * The perpendicular direction to this direction.
     *
     * [Horizontal] returns [Vertical], and [Vertical] returns [Horizontal].
     */
    fun perpendicular(): Direction = when (this) {
        Horizontal -> Vertical
        Vertical -> Horizontal
    }

    companion object {
        /** The default direction (Vertical) */
        val default: Direction = Vertical

        /** Parse a direction from a string */
        fun fromString(value: String): Direction = when (value) {
            "Horizontal" -> Horizontal
            "Vertical" -> Vertical
            else -> throw IllegalArgumentException("Unknown direction: $value")
        }
    }
}

// Tests
class DirectionTest {

    @Test
    fun testDirectionToString() {
        assertEquals(Direction.Horizontal.toString(), "Horizontal")
        assertEquals(Direction.Vertical.toString(), "Vertical")
    }

    @Test
    fun testDirectionFromStr() {
        assertEquals(Direction.fromString("Horizontal"), Direction.Horizontal)
        assertEquals(Direction.fromString("Vertical"), Direction.Vertical)
        assertFailsWith<IllegalArgumentException> {
            Direction.fromString("")
        }
    }

    @Test
    fun testPerpendicular() {
        assertEquals(Direction.Horizontal.perpendicular(), Direction.Vertical)
        assertEquals(Direction.Vertical.perpendicular(), Direction.Horizontal)
    }
}
