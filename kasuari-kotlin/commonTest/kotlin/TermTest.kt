package kasuari

import kotlin.test.Test
import kotlin.test.assertEquals

class TermTest {

    companion object {
        private val LEFT = Variable.fromId(0)
        private val RIGHT = Variable.fromId(1)
        private val LEFT_TERM = Term.fromVariable(LEFT)
        private val RIGHT_TERM = Term.fromVariable(RIGHT)
    }

    @Test
    fun testNew() {
        assertEquals(
            Term.new(LEFT, 2.0),
            Term(LEFT, 2.0)
        )
    }

    @Test
    fun testFromVariable() {
        assertEquals(
            Term.fromVariable(LEFT),
            Term(LEFT, 1.0)
        )
    }

    @Test
    fun mulF64() {
        assertEquals(
            LEFT_TERM * 2.0,
            Term(LEFT, 2.0)
        )
        assertEquals(
            2.0 * LEFT_TERM,
            Term(LEFT, 2.0)
        )
    }

    @Test
    fun mulF32() {
        assertEquals(
            LEFT_TERM * 2.0f,
            Term(LEFT, 2.0)
        )
        assertEquals(
            2.0f * LEFT_TERM,
            Term(LEFT, 2.0)
        )
    }

    @Test
    fun divF64() {
        assertEquals(
            LEFT_TERM / 2.0,
            Term(LEFT, 0.5)
        )
    }

    @Test
    fun divF32() {
        assertEquals(
            LEFT_TERM / 2.0f,
            Term(LEFT, 0.5)
        )
    }

    @Test
    fun addF64() {
        assertEquals(LEFT_TERM + 2.0, Expression.new(listOf(LEFT_TERM), 2.0))
        assertEquals(2.0 + LEFT_TERM, Expression.new(listOf(LEFT_TERM), 2.0))
    }

    @Test
    fun addF32() {
        assertEquals(LEFT_TERM + 2.0f, Expression.new(listOf(LEFT_TERM), 2.0))
        assertEquals(2.0f + LEFT_TERM, Expression.new(listOf(LEFT_TERM), 2.0))
    }

    @Test
    fun addTerm() {
        assertEquals(
            LEFT_TERM + RIGHT_TERM,
            Expression.fromTerms(listOf(LEFT_TERM, RIGHT_TERM))
        )
    }

    @Test
    fun addExpression() {
        assertEquals(
            LEFT_TERM + Expression.new(listOf(RIGHT_TERM), 1.0),
            Expression.new(listOf(LEFT_TERM, RIGHT_TERM), 1.0)
        )
    }

    @Test
    fun subF64() {
        assertEquals(LEFT_TERM - 2.0, Expression.new(listOf(LEFT_TERM), -2.0))
        assertEquals(2.0 - LEFT_TERM, Expression.new(listOf(-LEFT_TERM), 2.0))
    }

    @Test
    fun subF32() {
        assertEquals(LEFT_TERM - 2.0f, Expression.new(listOf(LEFT_TERM), -2.0))
        assertEquals(2.0f - LEFT_TERM, Expression.new(listOf(-LEFT_TERM), 2.0))
    }

    @Test
    fun subTerm() {
        assertEquals(
            LEFT_TERM - RIGHT_TERM,
            Expression.fromTerms(listOf(LEFT_TERM, -RIGHT_TERM))
        )
    }

    @Test
    fun subExpression() {
        assertEquals(
            LEFT_TERM - Expression.new(listOf(RIGHT_TERM), 1.0),
            Expression.new(listOf(LEFT_TERM, -RIGHT_TERM), -1.0)
        )
    }

    @Test
    fun neg() {
        assertEquals(
            -LEFT_TERM,
            Term(LEFT, -1.0)
        )
    }

    @Test
    fun mulAssignF64() {
        val term = Term.fromVariable(LEFT)
        term *= 2.0
        assertEquals(term, Term(LEFT, 2.0))
    }

    @Test
    fun mulAssignF32() {
        val term = Term.fromVariable(LEFT)
        term *= 2.0f
        assertEquals(term, Term(LEFT, 2.0))
    }

    @Test
    fun divAssignF64() {
        val term = Term.fromVariable(LEFT)
        term /= 2.0
        assertEquals(term, Term(LEFT, 0.5))
    }

    @Test
    fun divAssignF32() {
        val term = Term.fromVariable(LEFT)
        term /= 2.0f
        assertEquals(term, Term(LEFT, 0.5))
    }
}
