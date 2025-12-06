package kasuari

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals

class ConstraintTest {

    @Test
    fun constraintNew() {
        val v = Variable.new()
        val expr = Expression.fromVariable(v)
        val constraint = Constraint.new(expr, RelationalOperator.Equal, Strength.REQUIRED)

        assertEquals(RelationalOperator.Equal, constraint.op())
        assertEquals(Strength.REQUIRED, constraint.strength())
    }

    @Test
    fun constraintIdentityEquality() {
        val v = Variable.new()
        val expr = Expression.fromVariable(v)

        val c1 = Constraint.new(expr, RelationalOperator.Equal, Strength.REQUIRED)
        val c2 = Constraint.new(expr, RelationalOperator.Equal, Strength.REQUIRED)

        // Same content but different constraints (identity-based equality)
        assertNotEquals(c1, c2)

        // Same constraint should be equal to itself
        assertEquals(c1, c1)
    }

    @Test
    fun constraintDslEqual() {
        val v = Variable.new()

        // v == 100.0 with REQUIRED strength
        val constraint: Constraint = v with WeightedRelation.EQ(Strength.REQUIRED) to 100.0

        assertEquals(RelationalOperator.Equal, constraint.op())
        assertEquals(Strength.REQUIRED, constraint.strength())
    }

    @Test
    fun constraintDslLessOrEqual() {
        val v = Variable.new()

        // v <= 100.0 with STRONG strength
        val constraint: Constraint = v with WeightedRelation.LE(Strength.STRONG) to 100.0

        assertEquals(RelationalOperator.LessOrEqual, constraint.op())
        assertEquals(Strength.STRONG, constraint.strength())
    }

    @Test
    fun constraintDslGreaterOrEqual() {
        val v = Variable.new()

        // v >= 100.0 with MEDIUM strength
        val constraint: Constraint = v with WeightedRelation.GE(Strength.MEDIUM) to 100.0

        assertEquals(RelationalOperator.GreaterOrEqual, constraint.op())
        assertEquals(Strength.MEDIUM, constraint.strength())
    }

    @Test
    fun constraintDslWithVariable() {
        val v1 = Variable.new()
        val v2 = Variable.new()

        // v1 == v2 with WEAK strength
        val constraint: Constraint = v1 with WeightedRelation.EQ(Strength.WEAK) to v2

        assertEquals(RelationalOperator.Equal, constraint.op())
        assertEquals(Strength.WEAK, constraint.strength())
    }

    @Test
    fun constraintDslWithTerm() {
        val v1 = Variable.new()
        val v2 = Variable.new()
        val term = v2 * 2.0

        // v1 == 2*v2 with REQUIRED strength
        val constraint: Constraint = v1 with WeightedRelation.EQ(Strength.REQUIRED) to term

        assertEquals(RelationalOperator.Equal, constraint.op())
        assertEquals(Strength.REQUIRED, constraint.strength())
    }

    @Test
    fun constraintDslWithExpression() {
        val v1 = Variable.new()
        val v2 = Variable.new()
        val expr = v2 + 10.0

        // v1 == v2 + 10 with STRONG strength
        val constraint: Constraint = v1 with WeightedRelation.EQ(Strength.STRONG) to expr

        assertEquals(RelationalOperator.Equal, constraint.op())
        assertEquals(Strength.STRONG, constraint.strength())
    }

    @Test
    fun constraintExpressionSubtraction() {
        val v = Variable.new()

        // The constraint DSL subtracts rhs from expression
        // v == 100.0 becomes v - 100.0 == 0
        val constraint: Constraint = v with WeightedRelation.EQ(Strength.REQUIRED) to 100.0

        // The expression should have the constant subtracted
        assertEquals(-100.0, constraint.expr().constant)
    }
}
