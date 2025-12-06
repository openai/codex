package kasuari

/**
 * The possible relational operators that a constraint can specify.
 *
 * These operators define the relationship between the left-hand side and
 * right-hand side of a constraint equation.
 *
 * ## Examples
 *
 * ```kotlin
 * // x <= 100 (less than or equal)
 * RelationalOperator.LessOrEqual
 *
 * // x == 50 (equal)
 * RelationalOperator.Equal
 *
 * // x >= 0 (greater than or equal)
 * RelationalOperator.GreaterOrEqual
 * ```
 *
 * @see Constraint
 * @see WeightedRelation
 */
enum class RelationalOperator {
    /** Less than or equal (`<=`). */
    LessOrEqual,

    /** Equal (`==`). */
    Equal,

    /** Greater than or equal (`>=`). */
    GreaterOrEqual;

    /**
     * Returns the mathematical symbol for this operator.
     *
     * @return The operator as a string: `"<="`, `"=="`, or `">="`.
     */
    override fun toString(): String = when (this) {
        LessOrEqual -> "<="
        Equal -> "=="
        GreaterOrEqual -> ">="
    }
}

/**
 * A relational operator combined with a constraint strength.
 *
 * This is part of the DSL syntax for specifying constraints. Use [WeightedRelation]
 * with the `with` and `to` infix functions to create constraints in a readable way.
 *
 * ## Creating Constraints
 *
 * The constraint DSL uses a fluent syntax:
 *
 * ```kotlin
 * val x = Variable.new()
 * val y = Variable.new()
 *
 * // x == 100 (required)
 * val c1 = x with WeightedRelation.EQ(Strength.REQUIRED) to 100.0
 *
 * // x + y <= 200 (strong)
 * val c2 = (x + y) with WeightedRelation.LE(Strength.STRONG) to 200.0
 *
 * // 2*x >= y (medium)
 * val c3 = (2.0 * x) with WeightedRelation.GE(Strength.MEDIUM) to y
 * ```
 *
 * ## Relation Types
 *
 * - [EQ] - Equality constraint (`==`)
 * - [LE] - Less-than-or-equal constraint (`<=`)
 * - [GE] - Greater-than-or-equal constraint (`>=`)
 *
 * @property strength The strength of the constraint.
 * @see Strength
 * @see Constraint
 * @see PartialConstraint
 */
sealed class WeightedRelation {
    /** The strength associated with this weighted relation. */
    abstract val strength: Strength

    /**
     * Equality constraint (`==`) with the specified strength.
     *
     * @property strength The constraint strength.
     */
    data class EQ(override val strength: Strength) : WeightedRelation()

    /**
     * Less-than-or-equal constraint (`<=`) with the specified strength.
     *
     * @property strength The constraint strength.
     */
    data class LE(override val strength: Strength) : WeightedRelation()

    /**
     * Greater-than-or-equal constraint (`>=`) with the specified strength.
     *
     * @property strength The constraint strength.
     */
    data class GE(override val strength: Strength) : WeightedRelation()

    /**
     * Converts this weighted relation to a pair of operator and strength.
     *
     * @return A [Pair] of [RelationalOperator] and [Strength].
     */
    fun toOperatorAndStrength(): Pair<RelationalOperator, Strength> = when (this) {
        is EQ -> RelationalOperator.Equal to strength
        is LE -> RelationalOperator.LessOrEqual to strength
        is GE -> RelationalOperator.GreaterOrEqual to strength
    }
}

// ============================================================================
// Constraint DSL infix functions
// ============================================================================
//
// Kotlin doesn't have operator overloading for BitOr on arbitrary types like Rust.
// Instead, we provide infix functions for building constraints.
// Usage: expression with WeightedRelation.EQ(strength) to otherExpression

/**
 * Creates a partial constraint from a [Double] constant and a [WeightedRelation].
 *
 * Use the `to` infix function on the resulting [PartialConstraint] to complete
 * the constraint.
 *
 * ```kotlin
 * // 100.0 == x (required)
 * val constraint = 100.0 with WeightedRelation.EQ(Strength.REQUIRED) to x
 * ```
 *
 * @param relation The weighted relation specifying operator and strength.
 * @return A [PartialConstraint] that can be completed with `to`.
 */
infix fun Double.with(relation: WeightedRelation): PartialConstraint =
    PartialConstraint(Expression.fromConstant(this), relation)

/**
 * Creates a partial constraint from a [Float] constant and a [WeightedRelation].
 *
 * @param relation The weighted relation specifying operator and strength.
 * @return A [PartialConstraint] that can be completed with `to`.
 * @see Double.with
 */
infix fun Float.with(relation: WeightedRelation): PartialConstraint =
    PartialConstraint(Expression.fromConstant(this.toDouble()), relation)

/**
 * Creates a partial constraint from a [Variable] and a [WeightedRelation].
 *
 * ```kotlin
 * val x = Variable.new()
 *
 * // x == 100 (required)
 * val constraint = x with WeightedRelation.EQ(Strength.REQUIRED) to 100.0
 * ```
 *
 * @param relation The weighted relation specifying operator and strength.
 * @return A [PartialConstraint] that can be completed with `to`.
 */
infix fun Variable.with(relation: WeightedRelation): PartialConstraint =
    PartialConstraint(Expression.fromVariable(this), relation)

/**
 * Creates a partial constraint from a [Term] and a [WeightedRelation].
 *
 * ```kotlin
 * val x = Variable.new()
 *
 * // 2*x <= 100 (strong)
 * val constraint = (2.0 * x) with WeightedRelation.LE(Strength.STRONG) to 100.0
 * ```
 *
 * @param relation The weighted relation specifying operator and strength.
 * @return A [PartialConstraint] that can be completed with `to`.
 */
infix fun Term.with(relation: WeightedRelation): PartialConstraint =
    PartialConstraint(Expression.fromTerm(this), relation)

/**
 * Creates a partial constraint from an [Expression] and a [WeightedRelation].
 *
 * ```kotlin
 * val x = Variable.new()
 * val y = Variable.new()
 *
 * // x + y >= 0 (medium)
 * val constraint = (x + y) with WeightedRelation.GE(Strength.MEDIUM) to 0.0
 * ```
 *
 * @param relation The weighted relation specifying operator and strength.
 * @return A [PartialConstraint] that can be completed with `to`.
 */
infix fun Expression.with(relation: WeightedRelation): PartialConstraint =
    PartialConstraint(this, relation)
