package kasuari

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals

class VariableTest {

    companion object {
        private val LEFT = Variable.fromId(0)
        private val RIGHT = Variable.fromId(1)
        private val LEFT_TERM = Term.fromVariable(LEFT)
        private val RIGHT_TERM = Term.fromVariable(RIGHT)
    }

    @Test
    fun variableDefault() {
        assertNotEquals(LEFT, RIGHT)
    }

    @Test
    fun variableAddF64() {
        assertEquals(LEFT + 5.0, Expression.new(listOf(LEFT_TERM), 5.0))
        assertEquals(5.0 + LEFT, Expression.new(listOf(LEFT_TERM), 5.0))
    }

    @Test
    fun variableAddF32() {
        assertEquals(LEFT + 5.0f, Expression.new(listOf(LEFT_TERM), 5.0))
        assertEquals(5.0f + LEFT, Expression.new(listOf(LEFT_TERM), 5.0))
    }

    @Test
    fun variableAddVariable() {
        assertEquals(
            LEFT + RIGHT,
            Expression.new(listOf(LEFT_TERM, RIGHT_TERM), 0.0)
        )
    }

    @Test
    fun variableAddTerm() {
        assertEquals(
            LEFT + RIGHT_TERM,
            Expression.new(listOf(LEFT_TERM, RIGHT_TERM), 0.0)
        )
        assertEquals(
            LEFT_TERM + RIGHT,
            Expression.new(listOf(LEFT_TERM, RIGHT_TERM), 0.0)
        )
    }

    @Test
    fun variableAddExpression() {
        assertEquals(
            LEFT + Expression.fromTerm(RIGHT_TERM),
            Expression.new(listOf(LEFT_TERM, RIGHT_TERM), 0.0)
        )
        assertEquals(
            Expression.fromTerm(LEFT_TERM) + RIGHT,
            Expression.new(listOf(LEFT_TERM, RIGHT_TERM), 0.0)
        )
    }

    @Test
    fun variableAddAssign() {
        val expression = Expression.fromTerm(LEFT_TERM)
        expression += RIGHT
        assertEquals(
            expression,
            Expression.new(listOf(LEFT_TERM, RIGHT_TERM), 0.0)
        )
    }

    @Test
    fun variableSubF64() {
        assertEquals(LEFT - 5.0, Expression.new(listOf(LEFT_TERM), -5.0))
        assertEquals(5.0 - LEFT, Expression.new(listOf(-LEFT_TERM), 5.0))
    }

    @Test
    fun variableSubF32() {
        assertEquals(LEFT - 5.0f, Expression.new(listOf(LEFT_TERM), -5.0))
        assertEquals(5.0f - LEFT, Expression.new(listOf(-LEFT_TERM), 5.0))
    }

    @Test
    fun variableSubVariable() {
        assertEquals(
            LEFT - RIGHT,
            Expression.new(listOf(LEFT_TERM, -RIGHT_TERM), 0.0)
        )
    }

    @Test
    fun variableSubTerm() {
        assertEquals(
            LEFT - RIGHT_TERM,
            Expression.new(listOf(LEFT_TERM, -RIGHT_TERM), 0.0)
        )
        assertEquals(
            LEFT_TERM - RIGHT,
            Expression.new(listOf(LEFT_TERM, -RIGHT_TERM), 0.0)
        )
    }

    @Test
    fun variableSubExpression() {
        assertEquals(
            LEFT - Expression.fromTerm(RIGHT_TERM),
            Expression.new(listOf(LEFT_TERM, -RIGHT_TERM), 0.0)
        )
        assertEquals(
            Expression.fromTerm(LEFT_TERM) - RIGHT,
            Expression.new(listOf(LEFT_TERM, -RIGHT_TERM), 0.0)
        )
    }

    @Test
    fun variableSubAssign() {
        val expression = Expression.fromTerm(LEFT_TERM)
        expression -= RIGHT
        assertEquals(
            expression,
            Expression.new(listOf(LEFT_TERM, -RIGHT_TERM), 0.0)
        )
    }

    @Test
    fun variableMulF64() {
        assertEquals(LEFT * 5.0, Term.new(LEFT, 5.0))
        assertEquals(5.0 * LEFT, Term.new(LEFT, 5.0))
    }

    @Test
    fun variableMulF32() {
        assertEquals(LEFT * 5.0f, Term.new(LEFT, 5.0))
        assertEquals(5.0f * LEFT, Term.new(LEFT, 5.0))
    }

    @Test
    fun variableDivF64() {
        assertEquals(LEFT / 5.0, Term.new(LEFT, 1.0 / 5.0))
    }

    @Test
    fun variableDivF32() {
        assertEquals(LEFT / 5.0f, Term.new(LEFT, 1.0 / 5.0))
    }

    @Test
    fun variableNeg() {
        assertEquals(-LEFT, -LEFT_TERM)
    }
}
