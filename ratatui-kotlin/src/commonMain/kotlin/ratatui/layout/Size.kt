/**
 * A simple size struct for representing dimensions in the terminal.
 *
 * The width and height are stored as [UShort] values and represent the number of columns and rows
 * respectively. This is used throughout the layout system to represent dimensions of rectangular
 * areas and other layout elements.
 *
 * Size can be created from tuples, extracted from rectangular areas, or constructed directly.
 * It's commonly used in conjunction with [Position] to define rectangular areas.
 *
 * ## Construction
 *
 * - [new] - Create a new size from width and height
 * - Default constructor - Create with zero dimensions
 *
 * ## Conversion
 *
 * - [from] with `Pair<UShort, UShort>` - Create from tuple
 * - [from] with [Rect] - Create from Rect (uses width and height)
 * - [toPair] - Convert to `Pair<UShort, UShort>` tuple
 *
 * ## Computation
 *
 * - [area] - Compute the total number of cells covered by the size
 *
 * ## Examples
 *
 * ```kotlin
 * val size = Size.new(80u, 24u)
 * assertEquals(size.area(), 1920u)
 * val size = Size.from(Pair(80u, 24u))
 * val size = Size.from(Rect.new(0u, 0u, 80u, 24u))
 * assertEquals(size.area(), 1920u)
 * ```
 *
 * For comprehensive layout documentation and examples, see the layout module.
 */
package ratatui.layout

/**
 * A size representing dimensions in the terminal.
 *
 * @property width The width in columns
 * @property height The height in rows
 */
data class Size(
    val width: UShort,
    val height: UShort
) {

    companion object {
        /** A zero sized Size */
        val ZERO: Size = Size(0u, 0u)

        /** The minimum possible Size */
        val MIN: Size = ZERO

        /** The maximum possible Size */
        val MAX: Size = Size(UShort.MAX_VALUE, UShort.MAX_VALUE)

        /** Create a new Size */
        fun new(width: UShort, height: UShort): Size = Size(width, height)

        /** Create a Size from a pair of dimensions */
        fun from(pair: Pair<UShort, UShort>): Size = Size(pair.first, pair.second)

        /** Create a Size from a Rect (uses width and height) */
        fun from(rect: Rect): Size = rect.asSize()
    }

    /**
     * Compute the total area of the size as a [UInt].
     *
     * The multiplication uses [UInt] to avoid overflow when the width and height are at their
     * [UShort] maximum values.
     */
    fun area(): UInt = width.toUInt() * height.toUInt()

    /** Convert to a pair of dimensions */
    fun toPair(): Pair<UShort, UShort> = Pair(width, height)

    override fun toString(): String = "${width}x${height}"
}
