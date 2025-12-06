package kasuari

import kotlin.test.Test
import kotlin.test.assertEquals

class RemovalTest {

    @Test
    fun removeConstraint() {
        val (valueOf, updateValues) = newValues()

        val solver = Solver.new()

        val v = Variable.new()

        val constraint: Constraint = v with WeightedRelation.EQ(Strength.REQUIRED) to 100.0
        solver.addConstraint(constraint)
        updateValues(solver.fetchChanges())

        assertEquals(valueOf(v), 100.0)

        solver.removeConstraint(constraint)
        solver.addConstraint(v with WeightedRelation.EQ(Strength.REQUIRED) to 0.0)
        updateValues(solver.fetchChanges())

        assertEquals(valueOf(v), 0.0)
    }
}
