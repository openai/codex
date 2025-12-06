package kasuari

/**
 * Constraint strength for the Cassowary solver.
 *
 * Each constraint added to the solver has an associated strength specifying the precedence
 * the solver should impose when choosing which constraints to enforce. It will try to enforce all
 * constraints, but if that is impossible the lowest strength constraints are the first to be
 * violated.
 *
 * Strengths are simply real numbers. The strongest legal strength is 1,001,001,000.0. The weakest
 * is 0.0. For convenience, constants are declared for commonly used strengths. These are
 * [REQUIRED], [STRONG], [MEDIUM] and [WEAK]. Feel free to multiply these by other values
 * to get intermediate strengths. Note that the solver will clip given strengths to the legal
 * range.
 *
 * [REQUIRED] signifies a constraint that cannot be violated under any circumstance. Use this
 * special strength sparingly, as the solver will fail completely if it finds that not all of the
 * [REQUIRED] constraints can be satisfied. The other strengths represent fallible constraints.
 * These should be the most commonly used strengths for use cases where violating a constraint is
 * acceptable or even desired.
 *
 * The solver will try to get as close to satisfying the constraints it violates as possible,
 * strongest first. This behaviour can be used (for example) to provide a "default" value for a
 * variable should no other stronger constraints be put upon it.
 *
 * ## Predefined Strengths
 *
 * ```kotlin
 * Strength.REQUIRED  // 1,001,001,000.0 - cannot be violated
 * Strength.STRONG    // 1,000,000.0
 * Strength.MEDIUM    // 1,000.0
 * Strength.WEAK      // 1.0
 * Strength.ZERO      // 0.0 - weakest possible
 * ```
 *
 * ## Creating Custom Strengths
 *
 * ```kotlin
 * // Using arithmetic operators
 * val veryStrong = Strength.STRONG * 2.0    // 2,000,000.0
 * val halfMedium = Strength.MEDIUM / 2.0    // 500.0
 *
 * // Using the create function for fine-grained control
 * // create(strong, medium, weak, multiplier)
 * val custom = Strength.create(1.0, 0.0, 0.0, 1.0)  // equivalent to STRONG
 * ```
 *
 * ## Using Strengths with Constraints
 *
 * ```kotlin
 * val x = Variable.new()
 *
 * // Required constraint - must be satisfied
 * val required = x with WeightedRelation.EQ(Strength.REQUIRED) to 100.0
 *
 * // Strong constraint - high priority but can be violated
 * val strong = x with WeightedRelation.GE(Strength.STRONG) to 0.0
 *
 * // Weak constraint - used for defaults/preferences
 * val weak = x with WeightedRelation.EQ(Strength.WEAK) to 50.0
 * ```
 *
 * @see Constraint
 * @see Solver
 */
data class Strength(private val value: Double) : Comparable<Strength> {

    companion object {
        /**
         * The required strength for a constraint.
         *
         * This is the strongest possible strength (1,001,001,000.0). Constraints with this
         * strength **cannot** be violated under any circumstance. Use sparingly - if the
         * solver cannot satisfy all required constraints simultaneously, it will fail.
         */
        val REQUIRED = Strength(1_001_001_000.0)

        /**
         * A strong strength for a constraint.
         *
         * This is weaker than [REQUIRED] but stronger than [MEDIUM] (1,000,000.0).
         * Use for constraints that should almost always be satisfied but can be
         * violated if necessary.
         */
        val STRONG = Strength(1_000_000.0)

        /**
         * A medium strength for a constraint.
         *
         * This is weaker than [STRONG] but stronger than [WEAK] (1,000.0).
         * Use for constraints of moderate importance.
         */
        val MEDIUM = Strength(1_000.0)

        /**
         * A weak strength for a constraint.
         *
         * This is weaker than [MEDIUM] but stronger than [ZERO] (1.0).
         * Use for default values or preferences that should yield to stronger constraints.
         */
        val WEAK = Strength(1.0)

        /**
         * The weakest possible strength for a constraint (0.0).
         *
         * This is weaker than [WEAK]. Constraints with this strength have no effect
         * on the solution.
         */
        val ZERO = Strength(0.0)

        /**
         * Creates a new strength with the given value, clipped to the legal range.
         *
         * The value will be coerced to the range [0.0, [REQUIRED].value()].
         *
         * @param value The desired strength value.
         * @return A new [Strength] with the value clipped to the legal range.
         */
        fun new(value: Double): Strength =
            Strength(value.coerceIn(0.0, REQUIRED.value()))

        /**
         * Creates a strength as a linear combination of [STRONG], [MEDIUM], and [WEAK] strengths.
         *
         * This allows fine-grained control over constraint priority. Each weight is multiplied
         * by the multiplier, clamped to the range [0.0, 1000.0], and then multiplied by the
         * corresponding base strength. The resulting components are summed.
         *
         * ```kotlin
         * // Equivalent to STRONG
         * val s1 = Strength.create(1.0, 0.0, 0.0, 1.0)
         *
         * // Stronger than STRONG but weaker than REQUIRED
         * val s2 = Strength.create(500.0, 0.0, 0.0, 1.0)
         *
         * // Between MEDIUM and STRONG
         * val s3 = Strength.create(0.5, 0.5, 0.0, 1.0)
         * ```
         *
         * @param strong Weight for the strong component (clamped to [0.0, 1000.0]).
         * @param medium Weight for the medium component (clamped to [0.0, 1000.0]).
         * @param weak Weight for the weak component (clamped to [0.0, 1000.0]).
         * @param multiplier Multiplier applied to all weights.
         * @return A new [Strength] computed from the weighted combination.
         */
        fun create(strong: Double, medium: Double, weak: Double, multiplier: Double): Strength {
            val strongComponent = (strong * multiplier).coerceIn(0.0, 1000.0) * STRONG.value()
            val mediumComponent = (medium * multiplier).coerceIn(0.0, 1000.0) * MEDIUM.value()
            val weakComponent = (weak * multiplier).coerceIn(0.0, 1000.0) * WEAK.value()
            return new(strongComponent + mediumComponent + weakComponent)
        }
    }

    /**
     * Returns the numeric value of this strength.
     *
     * @return The strength value as a [Double].
     */
    fun value(): Double = value

    override fun compareTo(other: Strength): Int = value.compareTo(other.value)
}

// ============================================================================
// Operator overloading for Strength
// ============================================================================

/** Add two strengths together, clipping the result to the legal range */
operator fun Strength.plus(rhs: Strength): Strength =
    Strength.new(this.value() + rhs.value())

/** Subtract one strength from another, clipping the result to the legal range */
operator fun Strength.minus(rhs: Strength): Strength =
    Strength.new(this.value() - rhs.value())

/** Multiply a strength by a scalar, clipping the result to the legal range */
operator fun Strength.times(rhs: Double): Strength =
    Strength.new(this.value() * rhs)

/** Multiply a scalar by a strength, clipping the result to the legal range */
operator fun Double.times(rhs: Strength): Strength =
    Strength.new(this * rhs.value())

/** Multiply a strength by a scalar (Float), clipping the result to the legal range */
operator fun Strength.times(rhs: Float): Strength =
    Strength.new(this.value() * rhs.toDouble())

/** Multiply a scalar (Float) by a strength, clipping the result to the legal range */
operator fun Float.times(rhs: Strength): Strength =
    Strength.new(this.toDouble() * rhs.value())

/** Divide a strength by a scalar, clipping the result to the legal range */
operator fun Strength.div(rhs: Double): Strength =
    Strength.new(this.value() / rhs)

/** Divide a strength by a scalar (Float), clipping the result to the legal range */
operator fun Strength.div(rhs: Float): Strength =
    Strength.new(this.value() / rhs.toDouble())
