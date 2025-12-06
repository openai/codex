/**
 * A constraint that defines the size of a layout element.
 *
 * Constraints are the core mechanism for defining how space should be allocated within a
 * [Layout]. They can specify fixed sizes (length), proportional sizes
 * (percentage, ratio), size limits (min, max), or proportional fill values for layout elements.
 * Relative constraints (percentage, ratio) are calculated relative to the entire space being
 * divided, rather than the space available after applying more fixed constraints (min, max,
 * length).
 *
 * Constraints are prioritized in the following order:
 *
 * 1. [Constraint.Min]
 * 2. [Constraint.Max]
 * 3. [Constraint.Length]
 * 4. [Constraint.Percentage]
 * 5. [Constraint.Ratio]
 * 6. [Constraint.Fill]
 *
 * ## Size Calculation
 *
 * - [apply] - Apply the constraint to a length and return the resulting size
 *
 * ## Collection Creation
 *
 * - [fromLengths] - Create a collection of length constraints
 * - [fromRatios] - Create a collection of ratio constraints
 * - [fromPercentages] - Create a collection of percentage constraints
 * - [fromMaxes] - Create a collection of maximum constraints
 * - [fromMins] - Create a collection of minimum constraints
 * - [fromFills] - Create a collection of fill constraints
 *
 * ## Examples
 *
 * ```kotlin
 * // Create a layout with specified lengths for each element
 * val constraints = Constraint.fromLengths(listOf(10u, 20u, 10u))
 *
 * // Create a centered layout using ratio or percentage constraints
 * val constraints = Constraint.fromRatios(listOf(Pair(1u, 4u), Pair(1u, 2u), Pair(1u, 4u)))
 * val constraints = Constraint.fromPercentages(listOf(25u, 50u, 25u))
 *
 * // Create a centered layout with a minimum size constraint for specific elements
 * val constraints = Constraint.fromMins(listOf(0u, 100u, 0u))
 *
 * // Create a sidebar layout specifying maximum sizes for the columns
 * val constraints = Constraint.fromMaxes(listOf(30u, 170u))
 *
 * // Create a layout with fill proportional sizes for each element
 * val constraints = Constraint.fromFills(listOf(1u, 2u, 1u))
 * ```
 *
 * For comprehensive layout documentation and examples, see the layout module.
 */
package ratatui.layout


/**
 * A constraint that defines the size of a layout element.
 */
sealed class Constraint {

    /**
     * Applies a minimum size constraint to the element.
     *
     * The element size is set to at least the specified amount.
     */
    data class Min(val value: UShort) : Constraint()

    /**
     * Applies a maximum size constraint to the element.
     *
     * The element size is set to at most the specified amount.
     */
    data class Max(val value: UShort) : Constraint()

    /**
     * Applies a length constraint to the element.
     *
     * The element size is set to the specified amount.
     */
    data class Length(val value: UShort) : Constraint()

    /**
     * Applies a percentage of the available space to the element.
     *
     * Converts the given percentage to a floating-point value and multiplies that with area. This
     * value is rounded back to an integer as part of the layout split calculation.
     *
     * Note: As this value only accepts a [UShort], certain percentages that cannot be
     * represented exactly (e.g. 1/3) are not possible. You might want to use
     * [Constraint.Ratio] or [Constraint.Fill] in such cases.
     */
    data class Percentage(val value: UShort) : Constraint()

    /**
     * Applies a ratio of the available space to the element.
     *
     * Converts the given ratio to a floating-point value and multiplies that with area.
     * This value is rounded back to an integer as part of the layout split calculation.
     */
    data class Ratio(val numerator: UInt, val denominator: UInt) : Constraint()

    /**
     * Applies the scaling factor proportional to all other [Constraint.Fill] elements
     * to fill excess space.
     *
     * The element will only expand or fill into excess available space, proportionally matching
     * other [Constraint.Fill] elements while satisfying all other constraints.
     */
    data class Fill(val value: UShort) : Constraint()

    /**
     * Apply the constraint to a length and return the resulting size.
     *
     * @deprecated This method will be hidden in the next minor version.
     */
    @Deprecated("This method will be hidden in the next minor version.")
    fun apply(length: UShort): UShort {
        return when (this) {
            is Percentage -> {
                val p = value.toFloat() / 100.0f
                val len = length.toFloat()
                minOf(p * len, len).toInt().toUShort()
            }
            is Ratio -> {
                // avoid division by zero by using 1 when denominator is 0
                // this results in 0/0 -> 0 and x/0 -> x for x != 0
                val percentage = numerator.toFloat() / maxOf(denominator, 1u).toFloat()
                val len = length.toFloat()
                minOf(percentage * len, len).toInt().toUShort()
            }
            is Length -> minOf(length, value)
            is Fill -> minOf(length, value)
            is Max -> minOf(length, value)
            is Min -> maxOf(length, value)
        }
    }

    /** Check if this is a [Min] constraint */
    fun isMin(): Boolean = this is Min

    /** Check if this is a [Max] constraint */
    fun isMax(): Boolean = this is Max

    /** Check if this is a [Length] constraint */
    fun isLength(): Boolean = this is Length

    /** Check if this is a [Percentage] constraint */
    fun isPercentage(): Boolean = this is Percentage

    /** Check if this is a [Ratio] constraint */
    fun isRatio(): Boolean = this is Ratio

    /** Check if this is a [Fill] constraint */
    fun isFill(): Boolean = this is Fill

    override fun toString(): String = when (this) {
        is Percentage -> "Percentage($value)"
        is Ratio -> "Ratio($numerator, $denominator)"
        is Length -> "Length($value)"
        is Fill -> "Fill($value)"
        is Max -> "Max($value)"
        is Min -> "Min($value)"
    }

    companion object {
        /** The default constraint (Percentage(100)) */
        fun default(): Constraint = Percentage(100u)

        /** Create a Length constraint from a UShort */
        fun from(length: UShort): Constraint = Length(length)

        /**
         * Convert an iterable of lengths into a list of constraints.
         */
        fun fromLengths(lengths: Iterable<UShort>): List<Constraint> =
            lengths.map { Length(it) }

        /**
         * Convert an iterable of ratios into a list of constraints.
         */
        fun fromRatios(ratios: Iterable<Pair<UInt, UInt>>): List<Constraint> =
            ratios.map { (n, d) -> Ratio(n, d) }

        /**
         * Convert an iterable of percentages into a list of constraints.
         */
        fun fromPercentages(percentages: Iterable<UShort>): List<Constraint> =
            percentages.map { Percentage(it) }

        /**
         * Convert an iterable of maxes into a list of constraints.
         */
        fun fromMaxes(maxes: Iterable<UShort>): List<Constraint> =
            maxes.map { Max(it) }

        /**
         * Convert an iterable of mins into a list of constraints.
         */
        fun fromMins(mins: Iterable<UShort>): List<Constraint> =
            mins.map { Min(it) }

        /**
         * Convert an iterable of proportional factors into a list of constraints.
         */
        fun fromFills(proportionalFactors: Iterable<UShort>): List<Constraint> =
            proportionalFactors.map { Fill(it) }
    }
}
