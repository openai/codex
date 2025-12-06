/**
 * Position in the terminal coordinate system.
 *
 * The position is relative to the top left corner of the terminal window, with the top left corner
 * being (0, 0). The x axis is horizontal increasing to the right, and the y axis is vertical
 * increasing downwards.
 *
 * [Position] is used throughout the layout system to represent specific points in the terminal.
 * It can be created from coordinates, tuples, or extracted from rectangular areas.
 *
 * ## Construction
 *
 * - [new] - Create a new position from x and y coordinates
 * - Default constructor - Create at origin (0, 0)
 *
 * ## Conversion
 *
 * - [from] with `Pair<UShort, UShort>` - Create from tuple
 * - [from] with [Rect] - Create from Rect (uses top-left corner)
 * - [toPair] - Convert to `Pair<UShort, UShort>` tuple
 *
 * ## Examples
 *
 * ```kotlin
 * // the following are all equivalent
 * val position = Position(x = 1u, y = 2u)
 * val position = Position.new(1u, 2u)
 * val position = Position.from(Pair(1u, 2u))
 * val position = Position.from(Rect.new(1u, 2u, 3u, 4u))
 *
 * // position can be converted back into the components when needed
 * val (x, y) = position.toPair()
 * ```
 *
 * For comprehensive layout documentation and examples, see the layout module.
 */
package ratatui.layout


/**
 * A position in the terminal coordinate system.
 *
 * @property x The x coordinate of the position.
 *   The x coordinate is relative to the left edge of the terminal window, with the left edge being 0.
 * @property y The y coordinate of the position.
 *   The y coordinate is relative to the top edge of the terminal window, with the top edge being 0.
 */
data class Position(
    val x: UShort,
    val y: UShort
) : Comparable<Position> {

    companion object {
        /** Position at the origin, the top left edge at 0,0 */
        val ORIGIN: Position = Position(0u, 0u)

        /** Position at the minimum x and y values */
        val MIN: Position = ORIGIN

        /** Position at the maximum x and y values */
        val MAX: Position = Position(UShort.MAX_VALUE, UShort.MAX_VALUE)

        /** Create a new position */
        fun new(x: UShort, y: UShort): Position = Position(x, y)

        /** Create a position from a pair of coordinates */
        fun from(pair: Pair<UShort, UShort>): Position = Position(pair.first, pair.second)

        /** Create a position from a Rect (uses top-left corner) */
        fun from(rect: Rect): Position = rect.asPosition()
    }

    /** Convert to a pair of coordinates */
    fun toPair(): Pair<UShort, UShort> = Pair(x, y)

    override fun compareTo(other: Position): Int {
        val yCompare = y.compareTo(other.y)
        return if (yCompare != 0) yCompare else x.compareTo(other.x)
    }

    override fun toString(): String = "($x, $y)"
}
