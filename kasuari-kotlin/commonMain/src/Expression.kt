package kasuari

/**
 * An expression that can be the left hand or right hand side of a constraint equation.
 *
 * It is a linear combination of variables, i.e., a sum of variables weighted by coefficients,
 * plus an optional constant:
 *
 * ```
 * expression = term₁ + term₂ + ... + termₙ + constant
 *            = c₁×v₁ + c₂×v₂ + ... + cₙ×vₙ + constant
 * ```
 *
 * Expressions are the core building blocks for defining constraints. They can be created
 * from variables, terms, constants, or combinations thereof using arithmetic operators.
 *
 * ## Creating Expressions
 *
 * ```kotlin
 * val x = Variable.new()
 * val y = Variable.new()
 *
 * // From a constant
 * val expr1 = Expression.fromConstant(10.0)         // 10
 *
 * // From a variable
 * val expr2 = Expression.fromVariable(x)            // x
 *
 * // From a term
 * val expr3 = Expression.fromTerm(2.0 * x)          // 2x
 *
 * // Using operators
 * val expr4 = x + y + 5.0                           // x + y + 5
 * val expr5 = 2.0 * x - 3.0 * y + 10.0              // 2x - 3y + 10
 * ```
 *
 * ## Building Constraints from Expressions
 *
 * ```kotlin
 * val x = Variable.new()
 * val y = Variable.new()
 *
 * // Create constraints using the DSL
 * val constraint1 = x with WeightedRelation.EQ(Strength.REQUIRED) to 100.0
 * val constraint2 = (x + y) with WeightedRelation.LE(Strength.STRONG) to 200.0
 * ```
 *
 * @property terms The list of terms in this expression.
 * @property constant The constant value in this expression.
 * @see Variable
 * @see Term
 * @see Constraint
 */
data class Expression(
    /** The terms in the expression. */
    val terms: MutableList<Term>,
    /** The constant in the expression. */
    var constant: Double
) {
    /**
     * Creates a copy of this expression with an independent mutable terms list.
     *
     * Changes to the copy will not affect the original expression.
     *
     * @return A new [Expression] with the same terms and constant.
     */
    fun copy(): Expression = Expression(terms.toMutableList(), constant)

    companion object {
        /**
         * Creates a new [Expression] from a list of terms and a constant.
         *
         * ```
         * expression = term₁ + term₂ + ... + termₙ + constant
         * ```
         *
         * @param terms The terms in the expression.
         * @param constant The constant value.
         * @return A new [Expression].
         */
        fun new(terms: List<Term>, constant: Double): Expression =
            Expression(terms.toMutableList(), constant)

        /**
         * Constructs an expression that represents a constant without any terms.
         *
         * ```
         * expression = constant
         * ```
         *
         * @param constant The constant value.
         * @return A new [Expression] with no terms.
         */
        fun fromConstant(constant: Double): Expression =
            Expression(mutableListOf(), constant)

        /**
         * Constructs an expression from a single term.
         *
         * ```
         * expression = term
         * ```
         *
         * @param term The term to convert to an expression.
         * @return A new [Expression] containing the term.
         */
        fun fromTerm(term: Term): Expression =
            Expression(mutableListOf(term), 0.0)

        /**
         * Constructs an expression from a list of terms.
         *
         * ```
         * expression = term₁ + term₂ + ... + termₙ
         * ```
         *
         * @param terms The list of terms.
         * @return A new [Expression] with constant 0.0.
         */
        fun fromTerms(terms: List<Term>): Expression =
            Expression(terms.toMutableList(), 0.0)

        /**
         * Constructs an expression from an iterable of terms.
         *
         * This is equivalent to Rust's `impl FromIterator<Term> for Expression`.
         *
         * ```kotlin
         * val expr = Expression.fromTerms(terms.filter { it.coefficient != 0.0 })
         * ```
         *
         * @param terms An iterable of terms.
         * @return A new [Expression] with constant 0.0.
         */
        fun fromTerms(terms: Iterable<Term>): Expression =
            Expression(terms.toMutableList(), 0.0)

        /**
         * Constructs an expression from a sequence of terms.
         *
         * Useful for lazy evaluation of term collections.
         *
         * @param terms A sequence of terms.
         * @return A new [Expression] with constant 0.0.
         */
        fun fromTerms(terms: Sequence<Term>): Expression =
            Expression(terms.toMutableList(), 0.0)

        /**
         * Constructs an expression from a single variable.
         *
         * ```
         * expression = variable  (with coefficient 1.0)
         * ```
         *
         * @param variable The variable to convert to an expression.
         * @return A new [Expression] containing the variable.
         */
        fun fromVariable(variable: Variable): Expression =
            Expression(mutableListOf(Term.fromVariable(variable)), 0.0)
    }
}

// ============================================================================
// Operator overloading for Expression
// ============================================================================

/** Negates this expression, producing a new [Expression] with all terms and constant negated. */
operator fun Expression.unaryMinus(): Expression =
    Expression(
        terms.map { -it }.toMutableList(),
        if (constant == 0.0) 0.0 else -constant  // Normalize -0.0 to 0.0
    )

/** Multiplies this expression by a scalar, producing a new [Expression]. */
operator fun Expression.times(rhs: Double): Expression {
    val result = this.copy()
    result.constant *= rhs
    for (i in result.terms.indices) {
        result.terms[i] = result.terms[i] * rhs
    }
    return result
}

/** Multiplies a scalar by this expression, producing a new [Expression]. */
operator fun Double.times(rhs: Expression): Expression = rhs * this

/** Multiplies this expression by a scalar (Float), producing a new [Expression]. */
operator fun Expression.times(rhs: Float): Expression = this * rhs.toDouble()

/** Multiplies a scalar (Float) by this expression, producing a new [Expression]. */
operator fun Float.times(rhs: Expression): Expression = rhs * this.toDouble()

/** Divides this expression by a scalar, producing a new [Expression]. */
operator fun Expression.div(rhs: Double): Expression {
    val result = this.copy()
    result.constant /= rhs
    for (i in result.terms.indices) {
        result.terms[i] = result.terms[i] / rhs
    }
    return result
}

/** Divides this expression by a scalar (Float), producing a new [Expression]. */
operator fun Expression.div(rhs: Float): Expression = this / rhs.toDouble()

/** Adds a constant to this expression, producing a new [Expression]. */
operator fun Expression.plus(rhs: Double): Expression {
    val result = this.copy()
    result.constant += rhs
    return result
}

/** Adds an expression to this constant, producing a new [Expression]. */
operator fun Double.plus(rhs: Expression): Expression {
    val result = rhs.copy()
    result.constant += this
    return result
}

/** Adds a constant (Float) to this expression, producing a new [Expression]. */
operator fun Expression.plus(rhs: Float): Expression = this + rhs.toDouble()

/** Adds an expression to this constant (Float), producing a new [Expression]. */
operator fun Float.plus(rhs: Expression): Expression = this.toDouble() + rhs

/** Adds two expressions together, producing a new [Expression]. */
operator fun Expression.plus(rhs: Expression): Expression {
    val result = this.copy()
    result.terms.addAll(rhs.terms)
    result.constant += rhs.constant
    return result
}

/** Subtracts a constant from this expression, producing a new [Expression]. */
operator fun Expression.minus(rhs: Double): Expression {
    val result = this.copy()
    result.constant -= rhs
    return result
}

/** Subtracts an expression from this constant, producing a new [Expression]. */
operator fun Double.minus(rhs: Expression): Expression {
    val negated = -rhs
    negated.constant += this
    return negated
}

/** Subtracts a constant (Float) from this expression, producing a new [Expression]. */
operator fun Expression.minus(rhs: Float): Expression = this - rhs.toDouble()

/** Subtracts an expression from this constant (Float), producing a new [Expression]. */
operator fun Float.minus(rhs: Expression): Expression = this.toDouble() - rhs

/** Subtracts another expression from this expression, producing a new [Expression]. */
operator fun Expression.minus(rhs: Expression): Expression {
    val result = this.copy()
    val negated = -rhs
    result.terms.addAll(negated.terms)
    result.constant += negated.constant
    return result
}

// ============================================================================
// Compound assignment operators for Expression
// ============================================================================

/** Multiplies this expression by a scalar in place. */
operator fun Expression.timesAssign(rhs: Double) {
    constant *= rhs
    for (i in terms.indices) {
        terms[i] = terms[i] * rhs
    }
}

/** Multiplies this expression by a scalar (Float) in place. */
operator fun Expression.timesAssign(rhs: Float) {
    this *= rhs.toDouble()
}

/** Divides this expression by a scalar in place. */
operator fun Expression.divAssign(rhs: Double) {
    constant /= rhs
    for (i in terms.indices) {
        terms[i] = terms[i] / rhs
    }
}

/** Divides this expression by a scalar (Float) in place. */
operator fun Expression.divAssign(rhs: Float) {
    this /= rhs.toDouble()
}

/** Adds a constant to this expression in place. */
operator fun Expression.plusAssign(rhs: Double) {
    constant += rhs
}

/** Adds a constant (Float) to this expression in place. */
operator fun Expression.plusAssign(rhs: Float) {
    constant += rhs.toDouble()
}

/** Adds another expression to this expression in place. */
operator fun Expression.plusAssign(rhs: Expression) {
    terms.addAll(rhs.terms)
    constant += rhs.constant
}

/** Subtracts a constant from this expression in place. */
operator fun Expression.minusAssign(rhs: Double) {
    constant -= rhs
}

/** Subtracts a constant (Float) from this expression in place. */
operator fun Expression.minusAssign(rhs: Float) {
    constant -= rhs.toDouble()
}

/** Subtracts another expression from this expression in place. */
operator fun Expression.minusAssign(rhs: Expression) {
    val negated = -rhs
    terms.addAll(negated.terms)
    constant += negated.constant
}

// ============================================================================
// Conversion extensions
// ============================================================================

/**
 * Converts this [Double] to an [Expression] (constant-only expression).
 *
 * This is equivalent to Rust's `impl From<f64> for Expression`.
 *
 * @return An [Expression] with no terms and this value as the constant.
 */
fun Double.toExpression(): Expression = Expression.fromConstant(this)

/**
 * Converts this [Float] to an [Expression] (constant-only expression).
 *
 * @return An [Expression] with no terms and this value as the constant.
 */
fun Float.toExpression(): Expression = Expression.fromConstant(this.toDouble())
