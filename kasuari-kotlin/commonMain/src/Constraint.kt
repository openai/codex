package kasuari

/**
 * A constraint equation with an associated strength.
 *
 * A constraint represents a relationship between variables that the solver should try to satisfy.
 * It consists of:
 * - An [Expression] representing the left-hand side of the equation
 * - A [RelationalOperator] specifying the relationship (`<=`, `==`, `>=`)
 * - A [Strength] indicating the priority of satisfying this constraint
 *
 * The constraint equation is always normalized to have zero on the right-hand side:
 * ```
 * expression op 0.0
 * ```
 *
 * For example, `x + y == 10` is stored as `(x + y - 10) == 0`.
 *
 * ## Creating Constraints
 *
 * The recommended way to create constraints is using the DSL syntax:
 *
 * ```kotlin
 * val x = Variable.new()
 * val y = Variable.new()
 *
 * // x == 100 (required - must be satisfied)
 * val c1 = x with WeightedRelation.EQ(Strength.REQUIRED) to 100.0
 *
 * // x + y <= 200 (strong priority)
 * val c2 = (x + y) with WeightedRelation.LE(Strength.STRONG) to 200.0
 *
 * // x >= 0 (weak - preference, can be violated)
 * val c3 = x with WeightedRelation.GE(Strength.WEAK) to 0.0
 *
 * // x == y (variables on both sides)
 * val c4 = x with WeightedRelation.EQ(Strength.STRONG) to y
 * ```
 *
 * You can also create constraints directly using [Constraint.new]:
 *
 * ```kotlin
 * // x + y - 10 == 0 (equivalent to x + y == 10)
 * val constraint = Constraint.new(
 *     x + y - 10.0,
 *     RelationalOperator.Equal,
 *     Strength.REQUIRED
 * )
 * ```
 *
 * ## Identity Semantics
 *
 * Constraints are compared by identity (reference equality), not by value. Two constraints
 * with identical expressions, operators, and strengths are still considered different:
 *
 * ```kotlin
 * val c1 = x with WeightedRelation.EQ(Strength.REQUIRED) to 100.0
 * val c2 = x with WeightedRelation.EQ(Strength.REQUIRED) to 100.0
 *
 * c1 == c2  // false - different constraint instances
 * c1 == c1  // true - same instance
 * ```
 *
 * This is important when adding and removing constraints from the solver.
 *
 * @see Solver
 * @see Expression
 * @see Strength
 * @see WeightedRelation
 */
class Constraint private constructor(
    private val expression: Expression,
    private val operator: RelationalOperator,
    private val strength: Strength,
    /** Unique ID for identity-based equality. */
    private val id: Long
) {
    companion object {
        private var nextId: Long = 0

        /**
         * Constructs a new constraint from an expression, relational operator, and strength.
         *
         * The constraint represents the equation `expression op 0.0`. For equations with a
         * non-zero right-hand side, subtract it from the expression before calling this method.
         *
         * For example, to create the constraint `x + y == 10`:
         * ```kotlin
         * // The expression is (x + y - 10), which equals 0 when x + y == 10
         * val constraint = Constraint.new(
         *     x + y - 10.0,
         *     RelationalOperator.Equal,
         *     Strength.REQUIRED
         * )
         * ```
         *
         * @param expression The expression (left-hand side minus right-hand side).
         * @param operator The relational operator.
         * @param strength The constraint strength.
         * @return A new [Constraint].
         */
        fun new(
            expression: Expression,
            operator: RelationalOperator,
            strength: Strength
        ): Constraint = Constraint(expression, operator, strength, nextId++)
    }

    /**
     * Returns the expression of the constraint equation.
     *
     * This is the left-hand side of the normalized equation `expression op 0.0`.
     *
     * @return The constraint [Expression].
     */
    fun expr(): Expression = expression

    /**
     * Returns the relational operator governing the constraint.
     *
     * @return The [RelationalOperator] (`<=`, `==`, or `>=`).
     */
    fun op(): RelationalOperator = operator

    /**
     * Returns the strength of this constraint.
     *
     * The solver uses the strength to determine which constraints to violate first
     * when not all constraints can be satisfied simultaneously.
     *
     * @return The constraint [Strength].
     */
    fun strength(): Strength = strength

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is Constraint) return false
        return id == other.id
    }

    override fun hashCode(): Int = id.hashCode()

    override fun toString(): String =
        "Constraint(expression=$expression, operator=$operator, strength=$strength)"
}

/**
 * An intermediate type used in the constraint DSL.
 *
 * [PartialConstraint] is created when you use the `with` infix function on a variable,
 * term, or expression. It holds the left-hand side and the weighted relation, waiting
 * for the right-hand side to complete the constraint.
 *
 * You should not create instances of this class directly. Instead, use the DSL:
 *
 * ```kotlin
 * val x = Variable.new()
 *
 * // The `with` function returns a PartialConstraint
 * // The `to` function completes it into a Constraint
 * val constraint = x with WeightedRelation.EQ(Strength.REQUIRED) to 100.0
 * ```
 *
 * @property expression The left-hand side expression.
 * @property relation The weighted relation (operator + strength).
 * @see Constraint
 * @see WeightedRelation
 */
class PartialConstraint(
    /** The left-hand side expression of the constraint. */
    val expression: Expression,
    /** The weighted relation specifying operator and strength. */
    val relation: WeightedRelation
) {
    /**
     * Completes the constraint with a [Double] right-hand side.
     *
     * ```kotlin
     * val constraint = x with WeightedRelation.EQ(Strength.REQUIRED) to 100.0
     * ```
     *
     * @param rhs The right-hand side constant.
     * @return The completed [Constraint].
     */
    infix fun to(rhs: Double): Constraint {
        val (operator, strength) = relation.toOperatorAndStrength()
        return Constraint.new(expression - rhs, operator, strength)
    }

    /**
     * Completes the constraint with a [Float] right-hand side.
     *
     * @param rhs The right-hand side constant.
     * @return The completed [Constraint].
     * @see to(Double)
     */
    infix fun to(rhs: Float): Constraint = to(rhs.toDouble())

    /**
     * Completes the constraint with a [Variable] right-hand side.
     *
     * ```kotlin
     * val x = Variable.new()
     * val y = Variable.new()
     *
     * // x == y
     * val constraint = x with WeightedRelation.EQ(Strength.REQUIRED) to y
     * ```
     *
     * @param rhs The right-hand side variable.
     * @return The completed [Constraint].
     */
    infix fun to(rhs: Variable): Constraint {
        val (operator, strength) = relation.toOperatorAndStrength()
        return Constraint.new(expression - rhs, operator, strength)
    }

    /**
     * Completes the constraint with a [Term] right-hand side.
     *
     * ```kotlin
     * val x = Variable.new()
     * val y = Variable.new()
     *
     * // x == 2*y
     * val constraint = x with WeightedRelation.EQ(Strength.REQUIRED) to (2.0 * y)
     * ```
     *
     * @param rhs The right-hand side term.
     * @return The completed [Constraint].
     */
    infix fun to(rhs: Term): Constraint {
        val (operator, strength) = relation.toOperatorAndStrength()
        return Constraint.new(expression - rhs, operator, strength)
    }

    /**
     * Completes the constraint with an [Expression] right-hand side.
     *
     * ```kotlin
     * val x = Variable.new()
     * val y = Variable.new()
     * val z = Variable.new()
     *
     * // x == y + z
     * val constraint = x with WeightedRelation.EQ(Strength.REQUIRED) to (y + z)
     * ```
     *
     * @param rhs The right-hand side expression.
     * @return The completed [Constraint].
     */
    infix fun to(rhs: Expression): Constraint {
        val (operator, strength) = relation.toOperatorAndStrength()
        return Constraint.new(expression - rhs, operator, strength)
    }
}
