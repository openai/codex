/**
 * Alignment types for horizontal and vertical content positioning.
 *
 * For comprehensive layout documentation and examples, see the layout module.
 */
package ai.solace.coder.tui.layout

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/**
 * A type alias for [HorizontalAlignment].
 *
 * Prior to Ratatui 0.30.0, [HorizontalAlignment] was named `Alignment`. This alias is provided
 * for backwards compatibility. Because this type is used almost everywhere in Ratatui related apps
 * and libraries, it's unlikely that this alias will be removed in the future.
 */
typealias Alignment = HorizontalAlignment

/**
 * Horizontal content alignment within a layout area.
 *
 * Prior to Ratatui 0.30.0, this type was named `Alignment`. In Ratatui 0.30.0, the name was
 * changed to `HorizontalAlignment` to make it more descriptive. The old name is still available as
 * an alias for backwards compatibility.
 *
 * This type is used throughout Ratatui to control how content is positioned horizontally within
 * available space. It's commonly used with widgets to control text alignment, but can also be
 * used in layout calculations.
 *
 * For comprehensive layout documentation and examples, see the layout module.
 */
enum class HorizontalAlignment {
    Left,
    Center,
    Right;

    companion object {
        /** The default alignment (Left) */
        val default: HorizontalAlignment = Left

        /** Parse an alignment from a string */
        fun fromString(value: String): HorizontalAlignment = when (value) {
            "Left" -> Left
            "Center" -> Center
            "Right" -> Right
            else -> throw IllegalArgumentException("Unknown alignment: $value")
        }
    }
}

/**
 * Vertical content alignment within a layout area.
 *
 * This type is used to control how content is positioned vertically within available space.
 * It complements [HorizontalAlignment] to provide full 2D positioning control.
 *
 * For comprehensive layout documentation and examples, see the layout module.
 */
enum class VerticalAlignment {
    Top,
    Center,
    Bottom;

    companion object {
        /** The default alignment (Top) */
        val default: VerticalAlignment = Top

        /** Parse an alignment from a string */
        fun fromString(value: String): VerticalAlignment = when (value) {
            "Top" -> Top
            "Center" -> Center
            "Bottom" -> Bottom
            else -> throw IllegalArgumentException("Unknown alignment: $value")
        }
    }
}

// Tests
class AlignmentTest {

    @Test
    fun testAlignmentToString() {
        assertEquals(Alignment.Left.toString(), "Left")
        assertEquals(Alignment.Center.toString(), "Center")
        assertEquals(Alignment.Right.toString(), "Right")
    }

    @Test
    fun testAlignmentFromStr() {
        assertEquals(HorizontalAlignment.fromString("Left"), Alignment.Left)
        assertEquals(HorizontalAlignment.fromString("Center"), Alignment.Center)
        assertEquals(HorizontalAlignment.fromString("Right"), Alignment.Right)
        assertFailsWith<IllegalArgumentException> {
            HorizontalAlignment.fromString("")
        }
    }

    @Test
    fun testVerticalAlignmentToString() {
        assertEquals(VerticalAlignment.Top.toString(), "Top")
        assertEquals(VerticalAlignment.Center.toString(), "Center")
        assertEquals(VerticalAlignment.Bottom.toString(), "Bottom")
    }

    @Test
    fun testVerticalAlignmentFromStr() {
        assertEquals(VerticalAlignment.fromString("Top"), VerticalAlignment.Top)
        assertEquals(VerticalAlignment.fromString("Center"), VerticalAlignment.Center)
        assertEquals(VerticalAlignment.fromString("Bottom"), VerticalAlignment.Bottom)
        assertFailsWith<IllegalArgumentException> {
            VerticalAlignment.fromString("")
        }
    }
}
