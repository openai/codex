/**
 * A rectangular area in the terminal.
 *
 * A [Rect] represents a rectangular region in the terminal coordinate system, defined by its
 * top-left corner position and dimensions. This is the fundamental building block for all layout
 * operations and widget rendering in Ratatui.
 *
 * Rectangles are used throughout the layout system to define areas where widgets can be rendered.
 * They are typically created by [Layout] operations that divide terminal space, but can also be
 * manually constructed for specific positioning needs.
 *
 * The coordinate system uses the top-left corner as the origin (0, 0), with x increasing to the
 * right and y increasing downward. All measurements are in character cells.
 *
 * ## Construction and Conversion
 *
 * - [new] - Create a new rectangle from coordinates and dimensions
 * - [asPosition] - Convert to a position at the top-left corner
 * - [asSize] - Convert to a size representing the dimensions
 * - [from] with `Pair<Position, Size>` - Create from position and size tuple
 *
 * ## Geometry and Properties
 *
 * - [area] - Calculate the total area in character cells
 * - [isEmpty] - Check if the rectangle has zero area
 * - [left], [right], [top], [bottom] - Get edge coordinates
 *
 * ## Spatial Operations
 *
 * - [inner], [outer] - Apply margins to shrink or expand
 * - [offset] - Move the rectangle by a relative amount
 * - [resize] - Change the rectangle size while keeping the bottom/right in range
 * - [union] - Combine with another rectangle to create a bounding box
 * - [intersection] - Find the overlapping area with another rectangle
 * - [clamp] - Constrain the rectangle to fit within another
 *
 * ## Examples
 *
 * ```kotlin
 * val rect = Rect.new(1u, 2u, 3u, 4u)
 * assertEquals(rect, Rect(x = 1u, y = 2u, width = 3u, height = 4u))
 * ```
 *
 * For comprehensive layout documentation and examples, see the layout module.
 */
package ratatui.layout

/**
 * A rectangular area in the terminal.
 *
 * @property x The x coordinate of the top left corner of the Rect.
 * @property y The y coordinate of the top left corner of the Rect.
 * @property width The width of the Rect.
 * @property height The height of the Rect.
 */
data class Rect(
    val x: UShort,
    val y: UShort,
    val width: UShort,
    val height: UShort
) {

    companion object {
        /** A zero sized Rect at position 0,0 */
        val ZERO: Rect = Rect(0u, 0u, 0u, 0u)

        /** The minimum possible Rect */
        val MIN: Rect = ZERO

        /** The maximum possible Rect */
        val MAX: Rect = new(0u, 0u, UShort.MAX_VALUE, UShort.MAX_VALUE)

        /**
         * Creates a new Rect, with width and height limited to keep both bounds within UShort.
         *
         * If the width or height would cause the right or bottom coordinate to be larger than the
         * maximum value of UShort, the width or height will be clamped to keep the right or bottom
         * coordinate within UShort.
         */
        fun new(x: UShort, y: UShort, width: UShort, height: UShort): Rect {
            val clampedWidth = ((x + width).coerceAtMost(UShort.MAX_VALUE.toUInt()) - x).toUShort()
            val clampedHeight = ((y + height).coerceAtMost(UShort.MAX_VALUE.toUInt()) - y).toUShort()
            return Rect(x, y, clampedWidth, clampedHeight)
        }

        /** Create a Rect from a Position and Size */
        fun from(pair: Pair<Position, Size>): Rect = Rect(
            x = pair.first.x,
            y = pair.first.y,
            width = pair.second.width,
            height = pair.second.height
        )

        /** Create a Rect from a Size at origin (0, 0) */
        fun from(size: Size): Rect = Rect(
            x = 0u,
            y = 0u,
            width = size.width,
            height = size.height
        )

        /** Create a Rect that is empty (for Buffer.empty) - used by TestBackend */
        fun empty(rect: Rect): Rect = rect
    }

    /** The area of the Rect */
    fun area(): UInt = width.toUInt() * height.toUInt()

    /** Returns true if the Rect has no area */
    fun isEmpty(): Boolean = width.toInt() == 0 || height.toInt() == 0

    /** Returns the left coordinate of the Rect */
    fun left(): UShort = x

    /**
     * Returns the right coordinate of the Rect. This is the first coordinate outside of the Rect.
     *
     * If the right coordinate is larger than the maximum value of UShort, it will be clamped to
     * UShort.MAX_VALUE.
     */
    fun right(): UShort = (x + width).coerceAtMost(UShort.MAX_VALUE.toUInt()).toUShort()

    /** Returns the top coordinate of the Rect */
    fun top(): UShort = y

    /**
     * Returns the bottom coordinate of the Rect. This is the first coordinate outside of the Rect.
     *
     * If the bottom coordinate is larger than the maximum value of UShort, it will be clamped to
     * UShort.MAX_VALUE.
     */
    fun bottom(): UShort = (y + height).coerceAtMost(UShort.MAX_VALUE.toUInt()).toUShort()

    /**
     * Returns a new Rect inside the current one, with the given margin on each side.
     *
     * If the margin is larger than the Rect, the returned Rect will have no area.
     */
    fun inner(margin: Margin): Rect {
        val doubledMarginHorizontal = (margin.horizontal.toUInt() * 2u).toUShort()
        val doubledMarginVertical = (margin.vertical.toUInt() * 2u).toUShort()

        return if (width < doubledMarginHorizontal || height < doubledMarginVertical) {
            ZERO
        } else {
            Rect(
                x = (x + margin.horizontal).toUShort(),
                y = (y + margin.vertical).toUShort(),
                width = (width - doubledMarginHorizontal).toUShort(),
                height = (height - doubledMarginVertical).toUShort()
            )
        }
    }

    /**
     * Returns a new Rect outside the current one, with the given margin applied on each side.
     *
     * If the margin causes the Rect's bounds to be outside the range of a UShort, the Rect will
     * be truncated to keep the bounds within UShort.
     */
    fun outer(margin: Margin): Rect {
        val newX = (x.toInt() - margin.horizontal.toInt()).coerceAtLeast(0).toUShort()
        val newY = (y.toInt() - margin.vertical.toInt()).coerceAtLeast(0).toUShort()
        val newWidth = ((right().toUInt() + margin.horizontal.toUInt())
            .coerceAtMost(UShort.MAX_VALUE.toUInt()) - newX.toUInt()).toUShort()
        val newHeight = ((bottom().toUInt() + margin.vertical.toUInt())
            .coerceAtMost(UShort.MAX_VALUE.toUInt()) - newY.toUInt()).toUShort()
        return Rect(newX, newY, newWidth, newHeight)
    }

    /**
     * Moves the Rect without modifying its size.
     *
     * See [Offset] for details.
     */
    fun offset(offset: Offset): Rect {
        val newX = (x.toInt() + offset.x).coerceIn(0, UShort.MAX_VALUE.toInt() - width.toInt())
        val newY = (y.toInt() + offset.y).coerceIn(0, UShort.MAX_VALUE.toInt() - height.toInt())
        return copy(x = newX.toUShort(), y = newY.toUShort())
    }

    /**
     * Resizes the Rect, clamping to keep the right and bottom within UShort.MAX_VALUE.
     *
     * The position is preserved.
     */
    fun resize(size: Size): Rect {
        val newWidth = ((x.toUInt() + size.width.toUInt())
            .coerceAtMost(UShort.MAX_VALUE.toUInt()) - x.toUInt()).toUShort()
        val newHeight = ((y.toUInt() + size.height.toUInt())
            .coerceAtMost(UShort.MAX_VALUE.toUInt()) - y.toUInt()).toUShort()
        return copy(width = newWidth, height = newHeight)
    }

    /** Returns a new Rect that contains both the current one and the given one */
    fun union(other: Rect): Rect {
        val x1 = minOf(x, other.x)
        val y1 = minOf(y, other.y)
        val x2 = maxOf(right(), other.right())
        val y2 = maxOf(bottom(), other.bottom())
        return Rect(
            x = x1,
            y = y1,
            width = (x2 - x1).toUShort(),
            height = (y2 - y1).toUShort()
        )
    }

    /**
     * Returns a new Rect that is the intersection of the current one and the given one.
     *
     * If the two Rects do not intersect, the returned Rect will have no area.
     */
    fun intersection(other: Rect): Rect {
        val x1 = maxOf(x, other.x)
        val y1 = maxOf(y, other.y)
        val x2 = minOf(right(), other.right())
        val y2 = minOf(bottom(), other.bottom())
        return Rect(
            x = x1,
            y = y1,
            width = (x2.toInt() - x1.toInt()).coerceAtLeast(0).toUShort(),
            height = (y2.toInt() - y1.toInt()).coerceAtLeast(0).toUShort()
        )
    }

    /** Returns true if the two Rects intersect */
    fun intersects(other: Rect): Boolean {
        return x < other.right() &&
                right() > other.x &&
                y < other.bottom() &&
                bottom() > other.y
    }

    /**
     * Returns true if the given position is inside the Rect.
     *
     * The position is considered inside the Rect if it is on the Rect's border.
     */
    fun contains(position: Position): Boolean {
        return position.x >= x &&
                position.x < right() &&
                position.y >= y &&
                position.y < bottom()
    }

    /**
     * Clamp this Rect to fit inside the other Rect.
     *
     * If the width or height of this Rect is larger than the other Rect, it will be clamped to
     * the other Rect's width or height.
     */
    fun clamp(other: Rect): Rect {
        val newWidth = minOf(width, other.width)
        val newHeight = minOf(height, other.height)
        val newX = x.toInt().coerceIn(
            other.x.toInt(),
            (other.right().toInt() - newWidth.toInt()).coerceAtLeast(other.x.toInt())
        ).toUShort()
        val newY = y.toInt().coerceIn(
            other.y.toInt(),
            (other.bottom().toInt() - newHeight.toInt()).coerceAtLeast(other.y.toInt())
        ).toUShort()
        return new(newX, newY, newWidth, newHeight)
    }

    /** Returns a [Position] with the same coordinates as this Rect */
    fun asPosition(): Position = Position(x, y)

    /** Converts the Rect into a [Size] */
    fun asSize(): Size = Size(width, height)

    /** Convert to a pair of position and size */
    fun toPair(): Pair<Position, Size> = Pair(asPosition(), asSize())

    override fun toString(): String = "${width}x${height}+${x}+${y}"

    /**
     * Returns a new Rect with the x coordinate indented by the given width.
     *
     * The width is reduced by the indent amount. If the indent is larger than the width,
     * the width becomes 0.
     */
    fun indentX(indent: UShort): Rect {
        val newX = (x + indent).coerceAtMost(UShort.MAX_VALUE.toUInt()).toUShort()
        val newWidth = if (indent >= width) 0u else (width - indent).toUShort()
        return copy(x = newX, width = newWidth)
    }
}

// Operator extensions for Rect + Offset
operator fun Rect.plus(offset: Offset): Rect = this.offset(offset)
operator fun Rect.minus(offset: Offset): Rect = this.offset(Offset(-offset.x, -offset.y))
