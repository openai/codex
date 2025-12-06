package kasuari

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFails
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class SolverTest {

    // ============================================================================
    // Basic constraint tests
    // ============================================================================

    @Test
    fun simpleEqualityConstraint() {
        val solver = Solver.new()
        val v = Variable.new()

        solver.addConstraint(v with WeightedRelation.EQ(Strength.REQUIRED) to 100.0)

        assertEquals(100.0, solver.getValue(v))
    }

    @Test
    fun twoVariableEquality() {
        val solver = Solver.new()
        val v1 = Variable.new()
        val v2 = Variable.new()

        // v1 == 100
        solver.addConstraint(v1 with WeightedRelation.EQ(Strength.REQUIRED) to 100.0)
        // v2 == v1
        solver.addConstraint(v2 with WeightedRelation.EQ(Strength.REQUIRED) to v1)

        assertEquals(100.0, solver.getValue(v1))
        assertEquals(100.0, solver.getValue(v2))
    }

    @Test
    fun hasConstraint() {
        val solver = Solver.new()
        val v = Variable.new()
        val constraint = v with WeightedRelation.EQ(Strength.REQUIRED) to 100.0

        assertFalse(solver.hasConstraint(constraint))

        solver.addConstraint(constraint)

        assertTrue(solver.hasConstraint(constraint))
    }

    @Test
    fun duplicateConstraint() {
        val solver = Solver.new()
        val v = Variable.new()
        val constraint = v with WeightedRelation.EQ(Strength.REQUIRED) to 100.0

        solver.addConstraint(constraint)

        assertFails {
            solver.addConstraint(constraint)
        }
    }

    // ============================================================================
    // Edit variable tests
    // ============================================================================

    @Test
    fun editVariable() {
        val solver = Solver.new()
        val v = Variable.new()

        solver.addEditVariable(v, Strength.STRONG)
        solver.suggestValue(v, 50.0)

        assertEquals(50.0, solver.getValue(v))
    }

    @Test
    fun editVariableWithConstraint() {
        val solver = Solver.new()
        val v = Variable.new()

        // Add a weak constraint to 0
        solver.addConstraint(v with WeightedRelation.EQ(Strength.WEAK) to 0.0)

        // Add a strong edit variable
        solver.addEditVariable(v, Strength.STRONG)
        solver.suggestValue(v, 100.0)

        // Strong edit should win over weak constraint
        assertEquals(100.0, solver.getValue(v))
    }

    @Test
    fun hasEditVariable() {
        val solver = Solver.new()
        val v = Variable.new()

        assertFalse(solver.hasEditVariable(v))

        solver.addEditVariable(v, Strength.STRONG)

        assertTrue(solver.hasEditVariable(v))
    }

    @Test
    fun removeEditVariable() {
        val solver = Solver.new()
        val v = Variable.new()

        solver.addEditVariable(v, Strength.STRONG)
        assertTrue(solver.hasEditVariable(v))

        solver.removeEditVariable(v)
        assertFalse(solver.hasEditVariable(v))
    }

    @Test
    fun badRequiredStrength() {
        val solver = Solver.new()
        val v = Variable.new()

        // Cannot add edit variable with REQUIRED strength
        assertFails {
            solver.addEditVariable(v, Strength.REQUIRED)
        }
    }

    // ============================================================================
    // Fetch changes tests
    // ============================================================================

    @Test
    fun fetchChanges() {
        val (valueOf, updateValues) = newValues()
        val solver = Solver.new()
        val v = Variable.new()

        solver.addConstraint(v with WeightedRelation.EQ(Strength.REQUIRED) to 100.0)
        updateValues(solver.fetchChanges())

        assertEquals(100.0, valueOf(v))
    }

    @Test
    fun fetchChangesMultiple() {
        val (valueOf, updateValues) = newValues()
        val solver = Solver.new()
        val v1 = Variable.new()
        val v2 = Variable.new()

        solver.addConstraint(v1 with WeightedRelation.EQ(Strength.REQUIRED) to 100.0)
        solver.addConstraint(v2 with WeightedRelation.EQ(Strength.REQUIRED) to 200.0)
        updateValues(solver.fetchChanges())

        assertEquals(100.0, valueOf(v1))
        assertEquals(200.0, valueOf(v2))
    }

    // ============================================================================
    // Inequality tests
    // ============================================================================

    @Test
    fun lessThanOrEqual() {
        val solver = Solver.new()
        val v = Variable.new()

        // v <= 100 (required)
        solver.addConstraint(v with WeightedRelation.LE(Strength.REQUIRED) to 100.0)
        // Prefer v to be as high as possible
        solver.addConstraint(v with WeightedRelation.EQ(Strength.WEAK) to 1000.0)

        // Should be constrained to 100
        assertEquals(100.0, solver.getValue(v))
    }

    @Test
    fun greaterThanOrEqual() {
        val solver = Solver.new()
        val v = Variable.new()

        // v >= 100 (required)
        solver.addConstraint(v with WeightedRelation.GE(Strength.REQUIRED) to 100.0)
        // Prefer v to be as low as possible
        solver.addConstraint(v with WeightedRelation.EQ(Strength.WEAK) to 0.0)

        // Should be constrained to 100
        assertEquals(100.0, solver.getValue(v))
    }

    // ============================================================================
    // Strength priority tests
    // ============================================================================

    @Test
    fun strengthPriority() {
        val solver = Solver.new()
        val v = Variable.new()

        // Weak constraint: v == 100
        solver.addConstraint(v with WeightedRelation.EQ(Strength.WEAK) to 100.0)
        // Strong constraint: v == 200
        solver.addConstraint(v with WeightedRelation.EQ(Strength.STRONG) to 200.0)

        // Strong should win
        assertEquals(200.0, solver.getValue(v))
    }

    // ============================================================================
    // Reset tests
    // ============================================================================

    @Test
    fun reset() {
        val solver = Solver.new()
        val v = Variable.new()
        val constraint = v with WeightedRelation.EQ(Strength.REQUIRED) to 100.0

        solver.addConstraint(constraint)
        assertTrue(solver.hasConstraint(constraint))

        solver.reset()

        assertFalse(solver.hasConstraint(constraint))
        assertEquals(0.0, solver.getValue(v))
    }

    // ============================================================================
    // Linear combination tests
    // ============================================================================

    @Test
    fun linearCombination() {
        val solver = Solver.new()
        val x = Variable.new()
        val y = Variable.new()
        val z = Variable.new()

        // x == 10
        solver.addConstraint(x with WeightedRelation.EQ(Strength.REQUIRED) to 10.0)
        // y == 20
        solver.addConstraint(y with WeightedRelation.EQ(Strength.REQUIRED) to 20.0)
        // z == x + y
        solver.addConstraint(z with WeightedRelation.EQ(Strength.REQUIRED) to (x + y))

        assertEquals(10.0, solver.getValue(x))
        assertEquals(20.0, solver.getValue(y))
        assertEquals(30.0, solver.getValue(z))
    }

    @Test
    fun scaledVariable() {
        val solver = Solver.new()
        val x = Variable.new()
        val y = Variable.new()

        // x == 10
        solver.addConstraint(x with WeightedRelation.EQ(Strength.REQUIRED) to 10.0)
        // y == 2 * x
        solver.addConstraint(y with WeightedRelation.EQ(Strength.REQUIRED) to (x * 2.0))

        assertEquals(10.0, solver.getValue(x))
        assertEquals(20.0, solver.getValue(y))
    }
}
