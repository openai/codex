package kasuari

import kotlin.test.Test
import kotlin.test.assertEquals

class ExpressionTest {

    companion object {
        private val LEFT = Variable.fromId(0)
        private val RIGHT = Variable.fromId(1)
        private val LEFT_TERM = Term.fromVariable(LEFT)
        private val RIGHT_TERM = Term.fromVariable(RIGHT)
    }

    // ============================================================================
    // Construction tests
    // ============================================================================

    @Test
    fun newExpression() {
        val expr = Expression.new(listOf(LEFT_TERM, RIGHT_TERM), 5.0)
        assertEquals(2, expr.terms.size)
        assertEquals(5.0, expr.constant)
    }

    @Test
    fun fromConstant() {
        val expr = Expression.fromConstant(10.0)
        assertEquals(0, expr.terms.size)
        assertEquals(10.0, expr.constant)
    }

    @Test
    fun fromTerm() {
        val expr = Expression.fromTerm(LEFT_TERM)
        assertEquals(1, expr.terms.size)
        assertEquals(LEFT_TERM, expr.terms[0])
        assertEquals(0.0, expr.constant)
    }

    @Test
    fun fromTerms() {
        val expr = Expression.fromTerms(listOf(LEFT_TERM, RIGHT_TERM))
        assertEquals(2, expr.terms.size)
        assertEquals(0.0, expr.constant)
    }

    @Test
    fun fromVariable() {
        val expr = Expression.fromVariable(LEFT)
        assertEquals(1, expr.terms.size)
        assertEquals(LEFT, expr.terms[0].variable)
        assertEquals(1.0, expr.terms[0].coefficient)
        assertEquals(0.0, expr.constant)
    }

    // ============================================================================
    // Negation tests
    // ============================================================================

    @Test
    fun negExpression() {
        val expr = Expression.new(listOf(LEFT_TERM, RIGHT_TERM), 5.0)
        val negated = -expr
        assertEquals(-1.0, negated.terms[0].coefficient)
        assertEquals(-1.0, negated.terms[1].coefficient)
        assertEquals(-5.0, negated.constant)
    }

    @Test
    fun negExpressionZeroConstant() {
        val expr = Expression.new(listOf(LEFT_TERM), 0.0)
        val negated = -expr
        assertEquals(0.0, negated.constant) // -0.0 should be normalized to 0.0
    }

    // ============================================================================
    // Multiplication tests
    // ============================================================================

    @Test
    fun mulF64() {
        val expr = Expression.new(listOf(LEFT_TERM), 2.0)
        val result = expr * 3.0
        assertEquals(3.0, result.terms[0].coefficient)
        assertEquals(6.0, result.constant)
    }

    @Test
    fun mulF32() {
        val expr = Expression.new(listOf(LEFT_TERM), 2.0)
        val result = expr * 3.0f
        assertEquals(3.0, result.terms[0].coefficient)
        assertEquals(6.0, result.constant)
    }

    @Test
    fun scalarMulF64() {
        val expr = Expression.new(listOf(LEFT_TERM), 2.0)
        val result = 3.0 * expr
        assertEquals(3.0, result.terms[0].coefficient)
        assertEquals(6.0, result.constant)
    }

    @Test
    fun scalarMulF32() {
        val expr = Expression.new(listOf(LEFT_TERM), 2.0)
        val result = 3.0f * expr
        assertEquals(3.0, result.terms[0].coefficient)
        assertEquals(6.0, result.constant)
    }

    // ============================================================================
    // Division tests
    // ============================================================================

    @Test
    fun divF64() {
        val expr = Expression.new(listOf(Term.new(LEFT, 4.0)), 8.0)
        val result = expr / 2.0
        assertEquals(2.0, result.terms[0].coefficient)
        assertEquals(4.0, result.constant)
    }

    @Test
    fun divF32() {
        val expr = Expression.new(listOf(Term.new(LEFT, 4.0)), 8.0)
        val result = expr / 2.0f
        assertEquals(2.0, result.terms[0].coefficient)
        assertEquals(4.0, result.constant)
    }

    // ============================================================================
    // Addition tests
    // ============================================================================

    @Test
    fun addF64() {
        val expr = Expression.new(listOf(LEFT_TERM), 2.0)
        val result = expr + 3.0
        assertEquals(5.0, result.constant)
    }

    @Test
    fun addF32() {
        val expr = Expression.new(listOf(LEFT_TERM), 2.0)
        val result = expr + 3.0f
        assertEquals(5.0, result.constant)
    }

    @Test
    fun scalarAddF64() {
        val expr = Expression.new(listOf(LEFT_TERM), 2.0)
        val result = 3.0 + expr
        assertEquals(5.0, result.constant)
    }

    @Test
    fun scalarAddF32() {
        val expr = Expression.new(listOf(LEFT_TERM), 2.0)
        val result = 3.0f + expr
        assertEquals(5.0, result.constant)
    }

    @Test
    fun addExpression() {
        val expr1 = Expression.new(listOf(LEFT_TERM), 2.0)
        val expr2 = Expression.new(listOf(RIGHT_TERM), 3.0)
        val result = expr1 + expr2
        assertEquals(2, result.terms.size)
        assertEquals(5.0, result.constant)
    }

    // ============================================================================
    // Subtraction tests
    // ============================================================================

    @Test
    fun subF64() {
        val expr = Expression.new(listOf(LEFT_TERM), 5.0)
        val result = expr - 3.0
        assertEquals(2.0, result.constant)
    }

    @Test
    fun subF32() {
        val expr = Expression.new(listOf(LEFT_TERM), 5.0)
        val result = expr - 3.0f
        assertEquals(2.0, result.constant)
    }

    @Test
    fun scalarSubF64() {
        val expr = Expression.new(listOf(LEFT_TERM), 2.0)
        val result = 5.0 - expr
        assertEquals(-1.0, result.terms[0].coefficient)
        assertEquals(3.0, result.constant)
    }

    @Test
    fun scalarSubF32() {
        val expr = Expression.new(listOf(LEFT_TERM), 2.0)
        val result = 5.0f - expr
        assertEquals(-1.0, result.terms[0].coefficient)
        assertEquals(3.0, result.constant)
    }

    @Test
    fun subExpression() {
        val expr1 = Expression.new(listOf(LEFT_TERM), 5.0)
        val expr2 = Expression.new(listOf(RIGHT_TERM), 3.0)
        val result = expr1 - expr2
        assertEquals(2, result.terms.size)
        assertEquals(1.0, result.terms[0].coefficient)
        assertEquals(-1.0, result.terms[1].coefficient)
        assertEquals(2.0, result.constant)
    }

    // ============================================================================
    // Copy test
    // ============================================================================

    @Test
    fun copyExpression() {
        val original = Expression.new(listOf(LEFT_TERM, RIGHT_TERM), 5.0)
        val copy = original.copy()

        // Modify copy
        copy.terms.add(Term.new(Variable.new(), 1.0))
        copy.constant = 10.0

        // Original should be unchanged
        assertEquals(2, original.terms.size)
        assertEquals(5.0, original.constant)

        // Copy should be modified
        assertEquals(3, copy.terms.size)
        assertEquals(10.0, copy.constant)
    }

    // ============================================================================
    // Compound assignment tests
    // ============================================================================

    @Test
    fun timesAssignF64() {
        val expr = Expression.new(listOf(Term.new(LEFT, 2.0)), 4.0)
        expr *= 3.0
        assertEquals(6.0, expr.terms[0].coefficient)
        assertEquals(12.0, expr.constant)
    }

    @Test
    fun timesAssignF32() {
        val expr = Expression.new(listOf(Term.new(LEFT, 2.0)), 4.0)
        expr *= 3.0f
        assertEquals(6.0, expr.terms[0].coefficient)
        assertEquals(12.0, expr.constant)
    }

    @Test
    fun divAssignF64() {
        val expr = Expression.new(listOf(Term.new(LEFT, 6.0)), 12.0)
        expr /= 2.0
        assertEquals(3.0, expr.terms[0].coefficient)
        assertEquals(6.0, expr.constant)
    }

    @Test
    fun divAssignF32() {
        val expr = Expression.new(listOf(Term.new(LEFT, 6.0)), 12.0)
        expr /= 2.0f
        assertEquals(3.0, expr.terms[0].coefficient)
        assertEquals(6.0, expr.constant)
    }

    @Test
    fun plusAssignF64() {
        val expr = Expression.new(listOf(LEFT_TERM), 5.0)
        expr += 3.0
        assertEquals(8.0, expr.constant)
    }

    @Test
    fun plusAssignF32() {
        val expr = Expression.new(listOf(LEFT_TERM), 5.0)
        expr += 3.0f
        assertEquals(8.0, expr.constant)
    }

    @Test
    fun plusAssignExpression() {
        val expr1 = Expression.new(listOf(LEFT_TERM), 2.0)
        val expr2 = Expression.new(listOf(RIGHT_TERM), 3.0)
        expr1 += expr2
        assertEquals(2, expr1.terms.size)
        assertEquals(5.0, expr1.constant)
    }

    @Test
    fun minusAssignF64() {
        val expr = Expression.new(listOf(LEFT_TERM), 5.0)
        expr -= 3.0
        assertEquals(2.0, expr.constant)
    }

    @Test
    fun minusAssignF32() {
        val expr = Expression.new(listOf(LEFT_TERM), 5.0)
        expr -= 3.0f
        assertEquals(2.0, expr.constant)
    }

    @Test
    fun minusAssignExpression() {
        val expr1 = Expression.new(listOf(LEFT_TERM), 5.0)
        val expr2 = Expression.new(listOf(RIGHT_TERM), 3.0)
        expr1 -= expr2
        assertEquals(2, expr1.terms.size)
        assertEquals(1.0, expr1.terms[0].coefficient)  // LEFT_TERM unchanged
        assertEquals(-1.0, expr1.terms[1].coefficient) // -RIGHT_TERM
        assertEquals(2.0, expr1.constant)
    }
}
