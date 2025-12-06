package kasuari

import kotlin.concurrent.atomics.AtomicLong
import kotlin.concurrent.atomics.ExperimentalAtomicApi

/**
 * Identifies a variable for the constraint solver.
 *
 * Each new variable is unique in the view of the solver, but copying or cloning the variable
 * produces a copy of the same variable. Variables are the fundamental building blocks of
 * constraint expressions.
 *
 * ## Creating Variables
 *
 * ```kotlin
 * val x = Variable.new()
 * val y = Variable.new()
 * // x and y are distinct variables
 * ```
 *
 * ## Building Expressions
 *
 * Variables support arithmetic operators to build constraint expressions:
 *
 * ```kotlin
 * val x = Variable.new()
 * val y = Variable.new()
 *
 * // Expressions with constants
 * val expr1 = x + 10.0      // x + 10
 * val expr2 = 2.0 * x       // 2x (returns Term)
 * val expr3 = x / 2.0       // 0.5x (returns Term)
 *
 * // Expressions with other variables
 * val expr4 = x + y         // x + y
 * val expr5 = x - y         // x - y
 *
 * // Negation
 * val negX = -x             // -x (returns Term)
 * ```
 *
 * @property id The unique identifier for this variable.
 * @see Term
 * @see Expression
 * @see Constraint
 */
@ConsistentCopyVisibility
data class Variable internal constructor(val id: Long) : Comparable<Variable> {

    override fun compareTo(other: Variable): Int = id.compareTo(other.id)

    override fun toString(): String = "Variable(id=$id)"

    companion object {
        @OptIn(ExperimentalAtomicApi::class)
        private val nextId = AtomicLong(0)

        /**
         * Produces a new unique variable for use in constraint solving.
         *
         * Each call to [new] returns a variable with a unique ID that will never
         * be reused, even across different solver instances.
         *
         * @return A new unique [Variable].
         */
        @OptIn(ExperimentalAtomicApi::class)
        fun new(): Variable = Variable(nextId.fetchAndAdd(1))

        /**
         * Creates a variable with a specific ID. For testing purposes only.
         */
        internal fun fromId(id: Long): Variable = Variable(id)
    }
}

// ============================================================================
// Operator overloading for Variable
// ============================================================================

/** Adds a constant to this variable, producing an [Expression]. */
operator fun Variable.plus(constant: Double): Expression = Term.from(this) + constant

/** Adds a variable to this constant, producing an [Expression]. */
operator fun Double.plus(variable: Variable): Expression = Term.from(variable) + this

/** Adds a constant (Float) to this variable, producing an [Expression]. */
operator fun Variable.plus(constant: Float): Expression = Term.from(this) + constant.toDouble()

/** Adds a variable to this constant (Float), producing an [Expression]. */
operator fun Float.plus(variable: Variable): Expression = Term.from(variable) + this.toDouble()

/** Adds two variables together, producing an [Expression]. */
operator fun Variable.plus(other: Variable): Expression = Term.from(this) + Term.from(other)

/** Adds a [Term] to this variable, producing an [Expression]. */
operator fun Variable.plus(term: Term): Expression = Term.from(this) + term

/** Adds a [Variable] to this term, producing an [Expression]. */
operator fun Term.plus(variable: Variable): Expression = this + Term.from(variable)

/** Adds an [Expression] to this variable, producing an [Expression]. */
operator fun Variable.plus(expression: Expression): Expression = Term.from(this) + expression

/** Adds a [Variable] to this expression, producing an [Expression]. */
operator fun Expression.plus(variable: Variable): Expression = this + Term.from(variable)

/** Negates this variable, producing a [Term] with coefficient -1. */
operator fun Variable.unaryMinus(): Term = -Term.from(this)

/** Subtracts a constant from this variable, producing an [Expression]. */
operator fun Variable.minus(constant: Double): Expression = Term.from(this) - constant

/** Subtracts a variable from this constant, producing an [Expression]. */
operator fun Double.minus(variable: Variable): Expression = this - Term.from(variable)

/** Subtracts a constant (Float) from this variable, producing an [Expression]. */
operator fun Variable.minus(constant: Float): Expression = Term.from(this) - constant.toDouble()

/** Subtracts a variable from this constant (Float), producing an [Expression]. */
operator fun Float.minus(variable: Variable): Expression = this.toDouble() - Term.from(variable)

/** Subtracts another variable from this variable, producing an [Expression]. */
operator fun Variable.minus(other: Variable): Expression = Term.from(this) - Term.from(other)

/** Subtracts a [Term] from this variable, producing an [Expression]. */
operator fun Variable.minus(term: Term): Expression = Term.from(this) - term

/** Subtracts a [Variable] from this term, producing an [Expression]. */
operator fun Term.minus(variable: Variable): Expression = this - Term.from(variable)

/** Subtracts an [Expression] from this variable, producing an [Expression]. */
operator fun Variable.minus(expression: Expression): Expression = Term.from(this) - expression

/** Subtracts a [Variable] from this expression, producing an [Expression]. */
operator fun Expression.minus(variable: Variable): Expression = this - Term.from(variable)

/** Multiplies this variable by a coefficient, producing a [Term]. */
operator fun Variable.times(coefficient: Double): Term = Term(this, coefficient)

/** Multiplies a coefficient by this variable, producing a [Term]. */
operator fun Double.times(variable: Variable): Term = Term(variable, this)

/** Multiplies this variable by a coefficient (Float), producing a [Term]. */
operator fun Variable.times(coefficient: Float): Term = Term(this, coefficient.toDouble())

/** Multiplies a coefficient (Float) by this variable, producing a [Term]. */
operator fun Float.times(variable: Variable): Term = Term(variable, this.toDouble())

/** Divides this variable by a coefficient, producing a [Term]. */
operator fun Variable.div(coefficient: Double): Term = Term(this, 1.0 / coefficient)

/** Divides this variable by a coefficient (Float), producing a [Term]. */
operator fun Variable.div(coefficient: Float): Term = Term(this, 1.0 / coefficient.toDouble())

/** Adds a [Variable] to this expression in place. */
operator fun Expression.plusAssign(variable: Variable) {
    this += Term.from(variable)
}

/** Subtracts a [Variable] from this expression in place. */
operator fun Expression.minusAssign(variable: Variable) {
    this -= Term.from(variable)
}

// ============================================================================
// Conversion extensions for Variable
// ============================================================================

/**
 * Converts this [Variable] to a [Term] with coefficient 1.0.
 *
 * This is equivalent to Rust's `impl From<Variable> for Term`.
 *
 * @return A [Term] representing this variable with coefficient 1.0.
 */
fun Variable.toTerm(): Term = Term.fromVariable(this)

/**
 * Converts this [Variable] to an [Expression].
 *
 * This is equivalent to Rust's `impl From<Variable> for Expression`.
 *
 * @return An [Expression] containing only this variable.
 */
fun Variable.toExpression(): Expression = Expression.fromVariable(this)
