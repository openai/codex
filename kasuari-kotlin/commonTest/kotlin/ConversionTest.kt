package kasuari

import kotlin.test.Test
import kotlin.test.assertEquals

/**
 * Tests for type conversion extensions (equivalent to Rust's From<T> trait implementations).
 */
class ConversionTest {

    // ============================================================================
    // Variable conversions
    // ============================================================================

    @Test
    fun variableToTerm() {
        val v = Variable.new()
        val term = v.toTerm()

        assertEquals(v, term.variable)
        assertEquals(1.0, term.coefficient)
    }

    @Test
    fun variableToExpression() {
        val v = Variable.new()
        val expr = v.toExpression()

        assertEquals(1, expr.terms.size)
        assertEquals(v, expr.terms[0].variable)
        assertEquals(1.0, expr.terms[0].coefficient)
        assertEquals(0.0, expr.constant)
    }

    // ============================================================================
    // Term conversions
    // ============================================================================

    @Test
    fun termToExpression() {
        val v = Variable.new()
        val term = Term.new(v, 2.5)
        val expr = term.toExpression()

        assertEquals(1, expr.terms.size)
        assertEquals(v, expr.terms[0].variable)
        assertEquals(2.5, expr.terms[0].coefficient)
        assertEquals(0.0, expr.constant)
    }

    // ============================================================================
    // Numeric conversions
    // ============================================================================

    @Test
    fun doubleToExpression() {
        val expr = 42.0.toExpression()

        assertEquals(0, expr.terms.size)
        assertEquals(42.0, expr.constant)
    }

    @Test
    fun floatToExpression() {
        val expr = 42.0f.toExpression()

        assertEquals(0, expr.terms.size)
        assertEquals(42.0, expr.constant)
    }

    // ============================================================================
    // Expression.fromTerms with Iterable/Sequence
    // ============================================================================

    @Test
    fun expressionFromIterable() {
        val v1 = Variable.new()
        val v2 = Variable.new()
        val terms = setOf(Term.new(v1, 1.0), Term.new(v2, 2.0))  // Set is Iterable but not List

        val expr = Expression.fromTerms(terms)

        assertEquals(2, expr.terms.size)
        assertEquals(0.0, expr.constant)
    }

    @Test
    fun expressionFromSequence() {
        val v1 = Variable.new()
        val v2 = Variable.new()
        val terms = sequenceOf(Term.new(v1, 1.0), Term.new(v2, 2.0))

        val expr = Expression.fromTerms(terms)

        assertEquals(2, expr.terms.size)
        assertEquals(0.0, expr.constant)
    }

    @Test
    fun expressionFromMappedCollection() {
        val variables = listOf(Variable.new(), Variable.new(), Variable.new())

        // This is the idiomatic Kotlin pattern equivalent to Rust's FromIterator
        val expr = Expression.fromTerms(variables.map { it.toTerm() })

        assertEquals(3, expr.terms.size)
        assertEquals(0.0, expr.constant)
    }

    @Test
    fun expressionFromFilteredSequence() {
        val v1 = Variable.new()
        val v2 = Variable.new()
        val v3 = Variable.new()

        // Lazy sequence with filter
        val terms = sequenceOf(
            Term.new(v1, 1.0),
            Term.new(v2, 0.0),  // Will be filtered out
            Term.new(v3, 3.0)
        ).filter { it.coefficient != 0.0 }

        val expr = Expression.fromTerms(terms)

        assertEquals(2, expr.terms.size)
    }

    // ============================================================================
    // Chained conversions
    // ============================================================================

    @Test
    fun chainedConversion() {
        val v = Variable.new()

        // Variable -> Term -> Expression
        val expr = v.toTerm().toExpression()

        assertEquals(1, expr.terms.size)
        assertEquals(v, expr.terms[0].variable)
        assertEquals(1.0, expr.terms[0].coefficient)
    }
}
